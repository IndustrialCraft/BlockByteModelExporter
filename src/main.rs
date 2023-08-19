use std::{collections::HashMap, str::FromStr};

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
    let mut cubes = HashMap::new();
    for element in json["elements"].members() {
        let (cube, id) = CubeElement::from_json(element);
        cubes.insert(id, cube);
    }

    let root_bone = Bone::children_from_json(
        &json["outliner"],
        &mut cubes,
        Vec3 {
            x: 0.,
            y: 0.,
            z: 0.,
        },
        "root".to_string(),
    );
    //println!("{root_bone:#?}");
    let mut data = Vec::new();
    root_bone.to_stream(&mut data);
    data.write_be(0u32).unwrap();
    std::fs::write("out.bbm", data).unwrap();
}
#[derive(Clone, Debug)]
struct Bone {
    child_bones: Vec<Bone>,
    cube_elements: Vec<CubeElement>,
    //animations: HashMap<u32, AnimationData>,
    origin: Vec3,
    name: String,
    //item_mapping: Vec<(u32, ItemElement)>,
}
impl Bone {
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
        data.write_be(0u32).unwrap(); //items
        data.write_be(0u32).unwrap(); //animation
    }
    pub fn children_from_json(
        json: &JsonValue,
        cubes: &mut HashMap<uuid::Uuid, CubeElement>,
        origin: Vec3,
        name: String,
    ) -> Self {
        let mut child_bones = Vec::new();
        let mut cube_elements = Vec::new();
        for child in json.members() {
            match child {
                JsonValue::String(id) => {
                    let uuid = uuid::Uuid::from_str(id.as_str()).unwrap();
                    cube_elements.push(cubes.remove(&uuid).unwrap())
                }
                JsonValue::Object(bone) => {
                    child_bones.push(Bone::from_json(&JsonValue::Object(bone.clone()), cubes));
                }
                _ => panic!(""),
            }
        }
        Bone {
            child_bones,
            cube_elements,
            origin,
            name,
        }
    }
    pub fn from_json(json: &JsonValue, cubes: &mut HashMap<uuid::Uuid, CubeElement>) -> Self {
        Self::children_from_json(
            &json["children"],
            cubes,
            Vec3::from_json_pos(&json["origin"]),
            json["name"].as_str().unwrap().to_string(),
        )
    }
}
#[derive(Clone, Debug)]
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
}
#[derive(Clone, Debug)]
struct Vec2 {
    x: f32,
    y: f32,
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
    pub fn from_json(json: &JsonValue) -> (Self, uuid::Uuid) {
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
                front: CubeElementFace::from_json(&faces["north"]),
                back: CubeElementFace::from_json(&faces["south"]),
                left: CubeElementFace::from_json(&faces["west"]),
                right: CubeElementFace::from_json(&faces["east"]),
                up: CubeElementFace::from_json(&faces["up"]),
                down: CubeElementFace::from_json(&faces["down"]),
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
    pub fn from_json(json: &JsonValue) -> Self {
        let uv = &json["uv"];
        CubeElementFace {
            u1: uv[0].as_f32().unwrap() / 16.,
            v1: uv[1].as_f32().unwrap() / 16.,
            u2: uv[2].as_f32().unwrap() / 16.,
            v2: uv[3].as_f32().unwrap() / 16.,
        }
    }
}
#[derive(Clone, Debug)]
struct ItemElement {
    position: Vec3,
    rotation: Vec3,
    origin: Vec3,
    size: Vec2,
}
#[derive(Clone, Debug)]
struct AnimationData {
    position: Vec<AnimationKeyframe>,
    rotation: Vec<AnimationKeyframe>,
    scale: Vec<AnimationKeyframe>,
}
#[derive(Clone, Debug)]
struct AnimationKeyframe {
    data: Vec3,
    time: f32,
}
fn write_string(data: &mut Vec<u8>, value: &String) {
    data.write_be(value.len() as u16).unwrap();
    for ch in value.as_bytes() {
        data.write_be(*ch).unwrap();
    }
}
