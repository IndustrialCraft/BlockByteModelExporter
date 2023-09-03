use std::{collections::HashMap, str::FromStr};

use either::Either;
use endio::BEWrite;
use json::JsonValue;

fn main() {
    let file_name = std::env::args().nth(1).expect("missing file name");
    let json = json::parse(
        std::fs::read_to_string(file_name)
            .expect("file not found")
            .as_str(),
    )
    .expect("malformed model file");
    let texture_resolution = &json["resolution"];
    let texture_resolution = (
        texture_resolution["width"].as_u32().unwrap(),
        texture_resolution["height"].as_u32().unwrap(),
    );
    let mut elements = HashMap::new();
    for element in json["elements"].members() {
        let name = element["name"].as_str().unwrap();
        let (id, cube) = if name.starts_with("item_") {
            let (element, id) = ItemElement::from_json(name.replacen("item_", "", 1), element);
            (id, Either::Right(element))
        } else {
            let (element, id) = CubeElement::from_json(element, &texture_resolution);
            (id, Either::Left(element))
        };
        elements.insert(id, cube);
    }

    let mut root_bone = Bone::children_from_json(
        &json["outliner"],
        &mut elements,
        Vec3 {
            x: 0.,
            y: 0.,
            z: 0.,
        },
        "root".to_string(),
        uuid::Uuid::from_u128(0),
    );
    let mut animation_data = Vec::new();
    for (animation_id, animation) in json["animations"].members().enumerate() {
        let name = animation["name"].as_str().unwrap();
        let length = animation["length"].as_f32().unwrap();
        animation_data.push((name, length));
        for animator in animation["animators"].entries() {
            let uuid = uuid::Uuid::from_str(animator.0).unwrap();
            let animation_data = root_bone
                .find_sub_bone(&uuid)
                .unwrap()
                .animation_data_for_id(animation_id as u32);
            for keyframes in animator.1["keyframes"].members() {
                let channel = keyframes["channel"].as_str().unwrap();
                animation_data.add_keyframe(
                    channel,
                    if channel != "rotation" {
                        Vec3::from_keyframe_pos(&keyframes["data_points"][0])
                    } else {
                        Vec3::from_keyframe_rot(&keyframes["data_points"][0])
                    },
                    keyframes["time"].as_f32().unwrap(),
                );
            }
        }
    }
    //println!("{root_bone:#?}");
    let mut data = Vec::new();
    root_bone.to_stream(&mut data);

    data.write_be(animation_data.len() as u32).unwrap();
    for animation in animation_data {
        write_string(&mut data, animation.0);
        data.write_be(animation.1).unwrap();
    }
    std::fs::write("out.bbm", data).unwrap();
}
#[derive(Clone, Debug)]
struct Bone {
    uuid: uuid::Uuid,
    child_bones: Vec<Bone>,
    cube_elements: Vec<CubeElement>,
    animations: HashMap<u32, AnimationData>,
    origin: Vec3,
    name: String,
    item_elements: Vec<ItemElement>,
}
impl Bone {
    pub fn find_sub_bone(&mut self, id: &uuid::Uuid) -> Option<&mut Bone> {
        if &self.uuid == id {
            return Some(self);
        }
        for child in &mut self.child_bones {
            let sub = child.find_sub_bone(id);
            if sub.is_some() {
                return sub;
            }
        }
        None
    }
    pub fn animation_data_for_id(&mut self, id: u32) -> &mut AnimationData {
        self.animations.entry(id).or_insert_with(|| AnimationData {
            position: Vec::new(),
            rotation: Vec::new(),
            scale: Vec::new(),
        })
    }
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        write_string(data, &self.name);
        self.origin.to_stream(data);
        data.write_be(self.child_bones.len() as u32).unwrap();
        for bone in &self.child_bones {
            bone.to_stream(data);
        }
        data.write_be(self.cube_elements.len() as u32).unwrap();
        for element in &self.cube_elements {
            element.to_stream(data);
        }
        data.write_be(self.item_elements.len() as u32).unwrap();
        for element in &self.item_elements {
            write_string(data, &element.name);
            element.to_stream(data);
        }

        data.write_be(self.animations.len() as u32).unwrap();
        for animation in &self.animations {
            data.write_be(*animation.0).unwrap();
            animation.1.to_stream(data);
        }
    }
    pub fn children_from_json(
        json: &JsonValue,
        elements: &mut HashMap<uuid::Uuid, Either<CubeElement, ItemElement>>,
        origin: Vec3,
        name: String,
        uuid: uuid::Uuid,
    ) -> Self {
        let mut child_bones = Vec::new();
        let mut cube_elements = Vec::new();
        let mut item_elements = Vec::new();
        for child in json.members() {
            match child {
                JsonValue::String(id) => {
                    let uuid = uuid::Uuid::from_str(id.as_str()).unwrap();
                    match elements.remove(&uuid).unwrap() {
                        Either::Left(cube) => {
                            cube_elements.push(cube);
                        }
                        Either::Right(item) => {
                            item_elements.push(item);
                        }
                    }
                }
                JsonValue::Object(bone) => {
                    child_bones.push(Bone::from_json(&JsonValue::Object(bone.clone()), elements));
                }
                _ => panic!(""),
            }
        }
        Bone {
            uuid,
            child_bones,
            cube_elements,
            origin,
            name,
            item_elements,
            animations: HashMap::new(),
        }
    }
    pub fn from_json(
        json: &JsonValue,
        elements: &mut HashMap<uuid::Uuid, Either<CubeElement, ItemElement>>,
    ) -> Self {
        Self::children_from_json(
            &json["children"],
            elements,
            Vec3::from_json_pos(&json["origin"]),
            json["name"].as_str().unwrap().to_string(),
            uuid::Uuid::from_str(json["uuid"].as_str().unwrap()).unwrap(),
        )
    }
}
#[derive(Clone, Debug, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}
impl Vec3 {
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        data.write_be(self.x).unwrap();
        data.write_be(self.y).unwrap();
        data.write_be(self.z).unwrap();
    }
    pub fn from_json_pos(json: &JsonValue) -> Self {
        Vec3 {
            x: json[0].as_f32().unwrap() / 16.,
            y: json[1].as_f32().unwrap() / 16.,
            z: json[2].as_f32().unwrap() / 16.,
        }
    }
    pub fn from_json_rot(json: &JsonValue) -> Self {
        Vec3 {
            x: json[0].as_f32().unwrap().to_radians(),
            y: json[1].as_f32().unwrap().to_radians(),
            z: json[2].as_f32().unwrap().to_radians(),
        }
    }
    pub fn from_keyframe_pos(json: &JsonValue) -> Self {
        let x = &json["x"];
        let y = &json["y"];
        let z = &json["z"];
        let x: f32 = x
            .as_f32()
            .unwrap_or(x.as_str().unwrap_or("").parse().unwrap_or(0.));
        let y: f32 = y
            .as_f32()
            .unwrap_or(y.as_str().unwrap_or("").parse().unwrap_or(0.));
        let z: f32 = z
            .as_f32()
            .unwrap_or(z.as_str().unwrap_or("").parse().unwrap_or(0.));
        Vec3 {
            x: x / 16.,
            y: y / 16.,
            z: z / 16.,
        }
    }
    pub fn from_keyframe_rot(json: &JsonValue) -> Self {
        let x = &json["x"];
        let y = &json["y"];
        let z = &json["z"];
        let x: f32 = x
            .as_f32()
            .unwrap_or(x.as_str().and_then(|v| v.parse().ok()).unwrap_or(0.));
        let y: f32 = y
            .as_f32()
            .unwrap_or(y.as_str().and_then(|v| v.parse().ok()).unwrap_or(0.));
        let z: f32 = z
            .as_f32()
            .unwrap_or(z.as_str().and_then(|v| v.parse().ok()).unwrap_or(0.));
        Vec3 {
            x: x.to_radians(),
            y: y.to_radians(),
            z: z.to_radians(),
        }
    }
}
#[derive(Clone, Debug)]
struct Vec2 {
    x: f32,
    y: f32,
}
impl Vec2 {
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        data.write_be(self.x).unwrap();
        data.write_be(self.y).unwrap();
    }
}
#[derive(Clone, Debug)]
struct CubeElement {
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
    origin: Vec3,
    front: CubeElementFace,
    back: CubeElementFace,
    left: CubeElementFace,
    right: CubeElementFace,
    up: CubeElementFace,
    down: CubeElementFace,
}
impl CubeElement {
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        self.position.to_stream(data);
        self.scale.to_stream(data);
        self.rotation.to_stream(data);
        self.origin.to_stream(data);
        self.front.to_stream(data);
        self.back.to_stream(data);
        self.left.to_stream(data);
        self.right.to_stream(data);
        self.up.to_stream(data);
        self.down.to_stream(data);
    }
    pub fn from_json(json: &JsonValue, resolution: &(u32, u32)) -> (Self, uuid::Uuid) {
        let from = Vec3::from_json_pos(&json["from"]);
        let to = Vec3::from_json_pos(&json["to"]);
        let rotation = &json["rotation"];
        let faces = &json["faces"];
        (
            CubeElement {
                scale: Vec3 {
                    x: to.x - from.x,
                    y: to.y - from.y,
                    z: to.z - from.z,
                },
                position: from,
                rotation: if rotation.is_null() {
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    }
                } else {
                    Vec3::from_json_rot(rotation)
                },
                origin: Vec3::from_json_pos(&json["origin"]),
                front: CubeElementFace::from_json(&faces["north"], resolution),
                back: CubeElementFace::from_json(&faces["south"], resolution),
                left: CubeElementFace::from_json(&faces["west"], resolution),
                right: CubeElementFace::from_json(&faces["east"], resolution),
                up: CubeElementFace::from_json(&faces["up"], resolution),
                down: CubeElementFace::from_json(&faces["down"], resolution),
            },
            uuid::Uuid::from_str(json["uuid"].as_str().unwrap()).unwrap(),
        )
    }
}
#[derive(Clone, Debug)]
struct CubeElementFace {
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
}
impl CubeElementFace {
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        data.write_be(self.u1).unwrap();
        data.write_be(self.v1).unwrap();
        data.write_be(self.u2).unwrap();
        data.write_be(self.v2).unwrap();
    }
    pub fn from_json(json: &JsonValue, resolution: &(u32, u32)) -> Self {
        let uv = &json["uv"];
        CubeElementFace {
            u1: uv[0].as_f32().unwrap() / resolution.0 as f32,
            v1: uv[1].as_f32().unwrap() / resolution.1 as f32,
            u2: uv[2].as_f32().unwrap() / resolution.0 as f32,
            v2: uv[3].as_f32().unwrap() / resolution.1 as f32,
        }
    }
}
#[derive(Clone, Debug)]
struct ItemElement {
    name: String,
    position: Vec3,
    rotation: Vec3,
    origin: Vec3,
    size: Vec2,
}
impl ItemElement {
    pub fn from_json(name: String, json: &JsonValue) -> (Self, uuid::Uuid) {
        let from = Vec3::from_json_pos(&json["from"]);
        let to = Vec3::from_json_pos(&json["to"]);
        let rotation = &json["rotation"];
        (
            ItemElement {
                name,
                rotation: if rotation.is_null() {
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    }
                } else {
                    Vec3::from_json_rot(rotation)
                },
                origin: Vec3::from_json_pos(&json["origin"]),
                size: Vec2 {
                    x: to.x - from.x,
                    y: to.y - from.y,
                },
                position: from,
            },
            uuid::Uuid::from_str(json["uuid"].as_str().unwrap()).unwrap(),
        )
    }
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        self.position.to_stream(data);
        self.rotation.to_stream(data);
        self.origin.to_stream(data);
        self.size.to_stream(data);
    }
}
#[derive(Clone, Debug)]
struct AnimationData {
    position: Vec<AnimationKeyframe>,
    rotation: Vec<AnimationKeyframe>,
    scale: Vec<AnimationKeyframe>,
}
impl AnimationData {
    pub fn to_stream(&self, data: &mut Vec<u8>) {
        Self::keyframes_to_stream(data, &self.position);
        Self::keyframes_to_stream(data, &self.rotation);
        Self::keyframes_to_stream(data, &self.scale);
    }
    pub fn add_keyframe(&mut self, channel: &str, data: Vec3, time: f32) {
        let keyframe = AnimationKeyframe { data, time };
        match channel {
            "position" => self.position.push(keyframe),
            "rotation" => self.rotation.push(keyframe),
            "scale" => self.scale.push(keyframe),
            _ => panic!("unknown keyframe type"),
        }
    }
    pub fn keyframes_to_stream(data: &mut Vec<u8>, keyframes: &Vec<AnimationKeyframe>) {
        data.write_be(keyframes.len() as u32).unwrap();
        let mut sorted_keyframes: Vec<AnimationKeyframe> = keyframes.iter().cloned().collect();
        sorted_keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        for keyframe in sorted_keyframes {
            keyframe.data.to_stream(data);
            data.write_be(keyframe.time).unwrap();
        }
    }
}
#[derive(Clone, Debug, Copy)]
struct AnimationKeyframe {
    data: Vec3,
    time: f32,
}
fn write_string(data: &mut Vec<u8>, value: &str) {
    data.write_be(value.len() as u16).unwrap();
    for ch in value.as_bytes() {
        data.write_be(*ch).unwrap();
    }
}
