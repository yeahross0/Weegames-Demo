use macroquad::prelude::*;
use macroquad::rand::ChooseRandom;

use c2::prelude::*;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    default::Default,
    error::Error,
    ops::Not,
    path::Path,
    str,
};

pub const FPS: f32 = 60.0;

pub type WeeResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    fn zero() -> Vec2 {
        Vec2::new(0.0, 0.0)
    }
    fn magnitude(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    fn unit(self) -> Vec2 {
        if self.magnitude() == 0.0 {
            Vec2::zero()
        } else {
            self / self.magnitude()
        }
    }
}
impl std::ops::Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}
impl std::ops::Add<Vec2> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}
impl std::ops::Sub<Vec2> for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}
impl std::ops::AddAssign<Vec2> for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl std::ops::SubAssign<Vec2> for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl std::ops::Mul<Vec2> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl std::ops::Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self * rhs.x, self * rhs.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AABB {
    pub min: Vec2,
    pub max: Vec2,
}

impl AABB {
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    fn move_position(self, pos: Vec2) -> AABB {
        AABB {
            min: self.min + pos,
            max: self.max + pos,
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq)]
struct Rect {
    x: f32, // centre x
    y: f32, // centre y
    w: f32,
    h: f32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Colour {
    fn rgb(r: f32, g: f32, b: f32) -> Colour {
        Colour { r, g, b, a: 1.0 }
    }

    fn black() -> Colour {
        Colour::rgb(0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    fn new(width: f32, height: f32) -> Size {
        Size { width, height }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flip {
    pub horizontal: bool,
    pub vertical: bool,
}

impl Default for Flip {
    fn default() -> Flip {
        Flip {
            horizontal: false,
            vertical: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerialiseObject {
    pub name: String,
    sprite: Sprite,
    position: Vec2,
    size: Size,
    angle: f32,
    origin: Option<Vec2>,
    collision_area: Option<AABB>,
    flip: Flip,
    layer: u8,
    pub switch: Switch,
    instructions: Vec<Instruction>,
}

impl Default for SerialiseObject {
    fn default() -> SerialiseObject {
        SerialiseObject {
            name: "".to_string(),
            sprite: Sprite::Colour(Colour::black()),
            position: Vec2::new(800.0, 450.0),
            size: Size::new(100.0, 100.0),
            angle: 0.0,
            origin: None,
            collision_area: None,
            flip: Flip::default(),
            layer: 0,
            switch: Switch::Off,
            instructions: Vec::new(),
        }
    }
}

impl SerialiseObject {
    pub fn replace_text(&mut self, text_replacements: &[(&str, String)]) {
        fn replace_text_in_action(action: &mut Action, text_replacements: &[(&str, String)]) {
            if let Action::DrawText { text, .. } = action {
                for (before, after) in text_replacements {
                    *text = text.replace(before, &after);
                }
            } else if let Action::Random { random_actions } = action {
                for action in random_actions {
                    replace_text_in_action(action, text_replacements);
                }
            }
        }

        for instruction in self.instructions.iter_mut() {
            for action in instruction.actions.iter_mut() {
                replace_text_in_action(action, &text_replacements);
            }
        }
    }

    fn into_object(self) -> Object {
        let switch = match self.switch {
            Switch::On => SwitchState::On,
            Switch::Off => SwitchState::Off,
        };

        let mut object = Object {
            sprite: self.sprite,
            position: self.position,
            size: self.size,
            angle: self.angle,
            origin: self.origin,
            collision_area: self.collision_area,
            flip: self.flip,
            layer: self.layer,
            switch,
            instructions: self.instructions,
            queued_motion: Vec::new(),
            active_motion: ActiveMotion::Stop,
            animation: AnimationStatus::None,
            timer: None,
        };
        for instruction in object.instructions.iter_mut() {
            for trigger in instruction.triggers.iter_mut() {
                if let Trigger::Time(When::Random { start, end }) = trigger {
                    *trigger = Trigger::Time(When::Exact {
                        time: rand::gen_range(*start, *end + 1),
                    });
                }
            }
        }

        object
    }
}

trait SerialiseObjectList {
    fn get_obj(&self, name: &str) -> WeeResult<&SerialiseObject>;

    fn sprites(&self) -> HashMap<&str, &Sprite>;
}

impl SerialiseObjectList for Vec<SerialiseObject> {
    fn get_obj(&self, name: &str) -> WeeResult<&SerialiseObject> {
        let index = self.iter().position(|o| o.name == name);
        index
            .map(|index| self.get(index))
            .flatten()
            .ok_or_else(|| format!("Couldn't find object with name {}", name).into())
    }

    fn sprites(&self) -> HashMap<&str, &Sprite> {
        let mut sprites = HashMap::new();
        for obj in self {
            sprites.insert(&*obj.name, &obj.sprite);
        }
        sprites
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerialiseMusic {
    pub filename: String,
    pub looped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetFiles {
    pub images: HashMap<String, String>,
    pub audio: HashMap<String, String>,
    pub music: Option<SerialiseMusic>,
    pub fonts: HashMap<String, FontLoadInfo>,
}

impl Default for AssetFiles {
    fn default() -> AssetFiles {
        AssetFiles {
            images: HashMap::new(),
            audio: HashMap::new(),
            music: None,
            fonts: HashMap::new(),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameType {
    Minigame,
    BossGame,
    Other,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum Length {
    Seconds(f32),
    Infinite,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum FrameCount {
    Frames(u32),
    Infinite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameData {
    format_version: String,
    pub published: bool,
    pub game_type: GameType,
    pub objects: Vec<SerialiseObject>,
    background: Vec<BackgroundPart>,
    pub asset_files: AssetFiles,
    length: Length,
    pub intro_text: Option<String>,
    pub attribution: String,
}

impl Default for GameData {
    fn default() -> GameData {
        GameData {
            format_version: "0.2".to_string(),
            published: false,
            game_type: GameType::Minigame,
            objects: Vec::new(),
            background: Vec::new(),
            asset_files: AssetFiles::default(),
            length: Length::Seconds(4.0),
            intro_text: None,
            attribution: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum When {
    Start,
    End,
    Exact { time: u32 },
    Random { start: u32, end: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum CollisionWith {
    Object { name: String },
    Area(AABB),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MouseOver {
    Object { name: String },
    Area(AABB),
    Anywhere,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
#[repr(usize)]
pub enum ButtonState {
    Up = 0,
    Down = 1,
    Press = 2,
    Release = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MouseInteraction {
    Button { state: ButtonState },
    Hover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum Input {
    Mouse {
        over: MouseOver,
        interaction: MouseInteraction,
    },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum WinStatus {
    Won,
    Lost,
    HasBeenWon,
    HasBeenLost,
    NotYetWon,
    NotYetLost,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum SwitchState {
    On,
    Off,
    SwitchedOn,
    SwitchedOff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum PropertyCheck {
    Switch(SwitchState),
    Sprite(Sprite),
    FinishedAnimation,
    Timer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum Trigger {
    Time(When),
    Collision(CollisionWith),
    Input(Input),
    WinStatus(WinStatus),
    Random { chance: f32 },
    CheckProperty { name: String, check: PropertyCheck },
    DifficultyLevel { level: u32 },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    Freeze,
    None,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum Angle {
    Current,
    Degrees(f32),
    Random { min: f32, max: f32 },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum CompassDirection {
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

impl CompassDirection {
    fn angle(self) -> f32 {
        match self {
            CompassDirection::Up => 0.0,
            CompassDirection::UpRight => 45.0,
            CompassDirection::Right => 90.0,
            CompassDirection::DownRight => 135.0,
            CompassDirection::Down => 180.0,
            CompassDirection::DownLeft => 225.0,
            CompassDirection::Left => 270.0,
            CompassDirection::UpLeft => 315.0,
        }
    }

    fn all_directions() -> Vec<CompassDirection> {
        vec![
            CompassDirection::Up,
            CompassDirection::UpRight,
            CompassDirection::Right,
            CompassDirection::DownRight,
            CompassDirection::Down,
            CompassDirection::DownLeft,
            CompassDirection::Left,
            CompassDirection::UpLeft,
        ]
    }

    fn to_vector(self, speed: Speed) -> Vec2 {
        vector_from_angle(self.angle(), speed)
    }
}

fn gen_in_range(min: f32, max: f32) -> f32 {
    if min > max {
        rand::gen_range(max, min)
    } else if max > min {
        rand::gen_range(min, max)
    } else {
        min
    }
}

fn angle_from_direction(direction: &MovementDirection, object: &Object) -> f32 {
    match direction {
        MovementDirection::Angle(angle) => match angle {
            Angle::Current => object.angle,
            Angle::Degrees(degrees) => *degrees,
            Angle::Random { min, max } => gen_in_range(*min, *max),
        },
        MovementDirection::Direction {
            possible_directions,
        } => {
            let possible_directions = if !possible_directions.is_empty() {
                possible_directions.iter().cloned().collect()
            } else {
                CompassDirection::all_directions()
            };
            let dir = possible_directions.choose().unwrap();
            dir.angle()
        }
    }
}

fn vector_from_angle(angle: f32, speed: Speed) -> Vec2 {
    let speed = speed.as_value();
    let angle = (angle - 90.0).to_radians();
    Vec2::new(speed * angle.cos(), speed * angle.sin())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MovementDirection {
    Angle(Angle),
    Direction {
        possible_directions: HashSet<CompassDirection>,
    },
}

impl MovementDirection {
    fn angle(&self, object: &Object) -> f32 {
        angle_from_direction(self, object)
    }
    fn to_vector(&self, object: &Object, speed: Speed) -> Vec2 {
        vector_from_angle(self.angle(object), speed)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum Speed {
    VerySlow,
    Slow,
    Normal,
    Fast,
    VeryFast,
    Value(f32),
}

impl Speed {
    fn as_value(self) -> f32 {
        match self {
            Speed::VerySlow => 4.0,
            Speed::Slow => 8.0,
            Speed::Normal => 12.0,
            Speed::Fast => 16.0,
            Speed::VeryFast => 20.0,
            Speed::Value(value) => value,
        }
    }

    fn to_animation_time(self) -> u32 {
        match self {
            Speed::VerySlow => 32,
            Speed::Slow => 16,
            Speed::Normal => 8,
            Speed::Fast => 4,
            Speed::VeryFast => 2,
            Speed::Value(value) => value as u32,
        }
    }
}

fn random_velocity(speed: Speed) -> Vec2 {
    let speed = speed.as_value();
    let random_speed = || rand::gen_range(-speed, speed);
    Vec2::new(random_speed(), random_speed())
}

fn clamp_position(position: &mut Vec2, area: AABB) {
    position.x = position.x.min(area.max.x).max(area.min.x);
    position.y = position.y.min(area.max.y).max(area.min.y);
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum RelativeTo {
    CurrentPosition,
    CurrentAngle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum JumpLocation {
    Point(Vec2),
    Area(AABB),
    Relative { to: RelativeTo, distance: Vec2 },
    Object { name: String },
    Mouse,
    ClampPosition { area: AABB },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum BounceDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MovementType {
    Wiggle,
    Insect,
    Reflect {
        initial_direction: MovementDirection,
        movement_handling: MovementHandling,
    },
    Bounce {
        initial_direction: Option<BounceDirection>,
    },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum MovementHandling {
    Anywhere,
    TryNotToOverlap,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
enum Target {
    Object { name: String },
    Mouse,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum TargetType {
    Follow,
    StopWhenReached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum Acceleration {
    Continuous {
        direction: MovementDirection,
        speed: Speed,
    },
    SlowDown {
        speed: Speed,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum Motion {
    GoStraight {
        direction: MovementDirection,
        speed: Speed,
    },
    JumpTo(JumpLocation),
    Roam {
        movement_type: MovementType,
        area: AABB,
        speed: Speed,
    },
    Swap {
        name: String,
    },
    Target {
        target: Target,
        target_type: TargetType,
        offset: Vec2,
        speed: Speed,
    },
    Accelerate(Acceleration),
    Stop,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum AnimationType {
    Loop,
    PlayOnce,
}

impl Not for AnimationType {
    type Output = AnimationType;

    fn not(self) -> Self::Output {
        match self {
            AnimationType::Loop => AnimationType::PlayOnce,
            AnimationType::PlayOnce => AnimationType::Loop,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum AngleSetter {
    Value(f32),
    Increase(f32),
    Decrease(f32),
    Match { name: String },
    Clamp { min: f32, max: f32 },
    RotateToObject { name: String },
    RotateToMouse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum SizeDifference {
    Value(Size),
    Percent(Size),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum SizeSetter {
    Value(Size),
    Grow(SizeDifference),
    Shrink(SizeDifference),
    Clamp { min: Size, max: Size },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum Switch {
    On,
    Off,
}

impl Not for Switch {
    type Output = Switch;

    fn not(self) -> Self::Output {
        match self {
            Switch::On => Switch::Off,
            Switch::Off => Switch::On,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum FlipSetter {
    Flip,
    SetFlip(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum LayerSetter {
    Value(u8),
    Increase,
    Decrease,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum PropertySetter {
    Sprite(Sprite),
    Angle(AngleSetter),
    Size(SizeSetter),
    Switch(Switch),
    Timer { time: u32 },
    FlipHorizontal(FlipSetter),
    FlipVertical(FlipSetter),
    Layer(LayerSetter),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
enum TextResize {
    MatchText,
    MatchObject,
}

impl Not for TextResize {
    type Output = TextResize;

    fn not(self) -> Self::Output {
        match self {
            TextResize::MatchText => TextResize::MatchObject,
            TextResize::MatchObject => TextResize::MatchText,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum JustifyText {
    Centre,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum Action {
    Win,
    Lose,
    Effect(Effect),
    Motion(Motion),
    PlaySound {
        name: String,
    },
    StopMusic,
    SetProperty(PropertySetter),
    Animate {
        animation_type: AnimationType,
        sprites: Vec<Sprite>,
        speed: Speed,
    },
    DrawText {
        text: String,
        font: String,
        colour: Colour,
        resize: TextResize,
        justify: JustifyText,
    },
    Random {
        random_actions: Vec<Action>,
    },
    EndEarly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Instruction {
    triggers: Vec<Trigger>,
    actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FontLoadInfo {
    pub filename: String,
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Sprite {
    Image { name: String },
    Colour(Colour),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackgroundPart {
    pub sprite: Sprite,
    pub area: AABB,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Animation {
    should_loop: bool,
    index: usize,
    sprites: Vec<Sprite>,
    speed: Speed,
    time_to_next_change: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
enum AnimationStatus {
    Animating(Animation),
    Finished,
    None,
}

impl AnimationStatus {
    fn start(animation_type: AnimationType, sprites: &[Sprite], speed: Speed) -> AnimationStatus {
        let should_loop = match animation_type {
            AnimationType::Loop => true,
            AnimationType::PlayOnce => false,
        };
        AnimationStatus::Animating(Animation {
            should_loop,
            sprites: sprites.to_vec(),
            index: 0,
            speed,
            time_to_next_change: speed.to_animation_time(),
        })
    }

    fn update(&mut self) -> Option<Sprite> {
        match self {
            AnimationStatus::Animating(animation) => {
                if animation.time_to_next_change == 0 {
                    if animation.sprites.is_empty() {
                    } else if animation.index == animation.sprites.len() - 1 {
                        if animation.should_loop {
                            animation.index = 0;
                            animation.time_to_next_change = animation.speed.to_animation_time();
                            return Some(animation.sprites[0].clone());
                        } else {
                            *self = AnimationStatus::Finished;
                        }
                    } else {
                        animation.index += 1;
                        animation.time_to_next_change = animation.speed.to_animation_time();
                        return Some(animation.sprites[animation.index].clone());
                    }
                } else {
                    animation.time_to_next_change -= 1;
                }
            }
            AnimationStatus::Finished => {
                *self = AnimationStatus::None;
            }
            AnimationStatus::None => {}
        };

        None
    }
}

#[derive(Clone, Debug)]
enum ActiveMotion {
    GoStraight {
        velocity: Vec2,
    },
    Roam {
        movement_type: ActiveRoam,
        area: AABB,
        speed: Speed,
    },
    Target {
        target: Target,
        target_type: TargetType,
        offset: Vec2,
        speed: Speed,
    },
    Accelerate {
        velocity: Vec2,
        acceleration: Vec2,
    },
    SlowDown {
        velocity: Vec2,
        deceleration: Vec2,
    },
    Stop,
}
#[derive(Clone, Debug)]
enum ActiveRoam {
    Wiggle,
    Insect {
        velocity: Vec2,
    },
    Reflect {
        velocity: Vec2,
        movement_handling: MovementHandling,
    },
    Bounce {
        velocity: Vec2,
        acceleration: f32,
        direction: BounceDirection,
        frames_in_bounce: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Object {
    pub sprite: Sprite,
    pub position: Vec2,
    pub size: Size,
    pub angle: f32,
    origin: Option<Vec2>,
    collision_area: Option<AABB>,
    pub flip: Flip,
    pub layer: u8,
    instructions: Vec<Instruction>,
    queued_motion: Vec<Motion>,
    active_motion: ActiveMotion,
    pub switch: SwitchState,
    pub timer: Option<u32>,
    animation: AnimationStatus,
}

impl Object {
    fn origin(&self) -> Vec2 {
        self.origin
            .unwrap_or_else(|| Vec2::new(self.half_width(), self.half_height()))
    }

    pub fn origin_in_world(&self) -> Vec2 {
        self.top_left() + self.origin()
    }

    pub fn half_width(&self) -> f32 {
        self.size.width / 2.0
    }

    pub fn half_height(&self) -> f32 {
        self.size.height / 2.0
    }

    fn top_left(&self) -> Vec2 {
        Vec2::new(
            self.position.x - self.half_width(),
            self.position.y - self.half_height(),
        )
    }

    fn trig_angle(&self) -> f32 {
        (self.angle - 90.0).to_radians()
    }

    fn bottom_right(&self) -> Vec2 {
        Vec2::new(
            self.position.x + self.half_width(),
            self.position.y + self.half_height(),
        )
    }

    fn collision_aabb(&self) -> AABB {
        match &self.collision_area {
            Some(mut area) => {
                if self.flip.horizontal {
                    let difference_from_left = area.min.x;
                    let difference_from_right = self.size.width - area.max.x;
                    area.min.x = difference_from_right;
                    area.max.x = self.size.width - difference_from_left;
                }
                if self.flip.vertical {
                    let difference_from_top = area.min.y;
                    let difference_from_bottom = self.size.height - area.max.y;
                    area.min.y = difference_from_bottom;
                    area.max.y = self.size.height - difference_from_top;
                }
                area.move_position(self.top_left())
            }
            None => AABB {
                min: self.top_left(),
                max: self.bottom_right(),
            },
        }
    }

    fn poly(&self) -> c2::Poly {
        let collision_aabb = self.collision_aabb();
        let origin = self.origin_in_world();
        let aabb = collision_aabb.move_position(-origin);
        let c2v = |x, y| c2::Vec2::new(x, y);
        let mut points = [
            c2v(aabb.min.x, aabb.min.y),
            c2v(aabb.max.x, aabb.min.y),
            c2v(aabb.max.x, aabb.max.y),
            c2v(aabb.min.x, aabb.max.y),
        ];

        let angle = self.angle.to_radians();
        let c = angle.cos();
        let s = angle.sin();
        for point in points.iter_mut() {
            *point = c2v(
                point.x() * c - point.y() * s + origin.x,
                point.x() * s + point.y() * c + origin.y,
            );
        }
        c2::Poly::from_slice(&points)
    }

    pub fn update_timer(&mut self) {
        self.timer = match self.timer {
            Some(time) => {
                if time == 0 {
                    None
                } else {
                    Some(time - 1)
                }
            }
            None => None,
        };
    }

    pub fn update_animation(&mut self) {
        if let Some(sprite) = self.animation.update() {
            self.sprite = sprite;
        }
    }

    pub fn update_switch(&mut self, old_switch: SwitchState) {
        if self.switch == SwitchState::SwitchedOn
            && (old_switch == SwitchState::SwitchedOn || old_switch == SwitchState::On)
        {
            self.switch = SwitchState::On;
        } else if self.switch == SwitchState::SwitchedOff
            && (old_switch == SwitchState::SwitchedOff || old_switch == SwitchState::Off)
        {
            self.switch = SwitchState::Off;
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct GameStatus {
    pub current: WinStatus,
    pub next_frame: WinStatus,
}

impl GameData {
    pub async fn load(filename: impl AsRef<Path>) -> WeeResult<GameData> {
        let json_string =
            macroquad::file::load_string(&filename.as_ref().to_string_lossy()).await?;

        json_from_str(&json_string)
    }
}

fn json_from_str<'a, T: Deserialize<'a>>(text: &'a str) -> WeeResult<T> {
    match serde_json::from_str(text) {
        Ok(data) => Ok(data),
        Err(error) => Err(Box::new(error)),
    }
}

pub type Objects = IndexMap<String, Object>;

trait ObjectList {
    fn get_obj(&self, name: &str) -> WeeResult<&Object>;

    fn from_serialised(objects: Vec<SerialiseObject>) -> Self;
}

impl ObjectList for Objects {
    fn get_obj(&self, name: &str) -> WeeResult<&Object> {
        self.get(name)
            .ok_or_else(|| format!("Couldn't find object with name {}", name).into())
    }

    fn from_serialised(objects: Vec<SerialiseObject>) -> Objects {
        let mut new_objects = Objects::new();

        for object in objects {
            new_objects.insert(object.name.clone(), object.into_object());
        }

        new_objects
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FrameInfo {
    total: FrameCount,
    pub ran: u32,
    pub steps_taken: u32,
    start_time: f64,
    to_run: u32,
}

impl FrameInfo {
    pub fn remaining(self) -> FrameCount {
        match self.total {
            FrameCount::Frames(frames) => {
                if frames < self.ran {
                    FrameCount::Frames(0)
                } else {
                    FrameCount::Frames((frames - self.ran).max(0))
                }
            }
            FrameCount::Infinite => FrameCount::Infinite,
        }
    }

    fn is_final(self) -> bool {
        self.remaining() == FrameCount::Frames(1)
    }
}

#[derive(Copy, Clone)]
pub struct Mouse {
    pub position: Vec2,
    pub state: ButtonState,
}

pub struct DrawnText {
    pub text: String,
    pub font: String,
    pub colour: Colour,
    //resize: TextResize,
    pub justify: JustifyText,
}

pub struct Game {
    pub objects: Objects,
    pub background: Vec<BackgroundPart>,
    pub frames: FrameInfo,
    pub status: GameStatus,
    pub effect: Effect,
    pub intro_text: String,
    pub drawn_text: HashMap<String, DrawnText>,
    pub difficulty: u32,
    pub has_music_finished: bool,
    pub end_early: bool,
}

impl Game {
    pub fn from_data(game_data: GameData) -> Game {
        Game {
            objects: Objects::from_serialised(game_data.objects),
            background: game_data.background,
            frames: FrameInfo {
                total: match game_data.length {
                    Length::Seconds(seconds) => FrameCount::Frames((seconds * FPS) as u32),
                    Length::Infinite => FrameCount::Infinite,
                },
                ran: 0,
                steps_taken: 0,
                start_time: macroquad::time::get_time(),
                to_run: 0,
            },
            status: GameStatus {
                current: WinStatus::NotYetWon,
                next_frame: WinStatus::NotYetWon,
            },
            effect: Effect::None,
            intro_text: game_data.intro_text.as_deref().unwrap_or("").to_string(),
            drawn_text: HashMap::new(),
            difficulty: 1,
            has_music_finished: false,
            end_early: false,
        }
    }

    pub fn update(&mut self, mouse: &Mouse) -> WeeResult<Vec<String>> {
        let mut played_sounds = Vec::new();
        let keys: Vec<String> = self.objects.keys().cloned().collect();
        match self.effect {
            Effect::None => {
                for name in keys.iter() {
                    let old_switch = self.objects[name].switch;

                    self.objects[name].update_timer();

                    let actions = self.check_triggers(name, &mouse)?;

                    let mut new_sounds = self.apply_actions(name, &actions, &mouse)?;
                    played_sounds.append(&mut new_sounds);

                    self.objects[name].update_animation();

                    self.move_object(name, &mouse)?;

                    self.objects[name].update_switch(old_switch);
                }
            }
            Effect::Freeze => {
                for name in keys.iter() {
                    self.objects[name].update_timer();

                    let actions = self.check_triggers(name, &mouse)?;

                    for action in actions {
                        if action == Action::EndEarly {
                            self.end_early = true;
                        }
                    }
                }
            }
        }

        Ok(played_sounds)
    }

    fn is_triggered(&self, name: &str, trigger: &Trigger, mouse: &Mouse) -> WeeResult<bool> {
        let is_point_in_area = |pos: Vec2, area: AABB| {
            pos.x >= area.min.x && pos.y >= area.min.y && pos.x < area.max.x && pos.y < area.max.y
        };
        let is_mouse_in_area = |mouse: &Mouse, area| is_point_in_area(mouse.position, area);
        let c2v = |v: Vec2| c2::Vec2::new(v.x, v.y);
        let triggered = match trigger {
            Trigger::Time(When::Start) => self.frames.ran == 0,
            Trigger::Time(When::End) => self.frames.is_final(),
            Trigger::Time(When::Exact { time }) => self.frames.ran == *time,
            Trigger::Time(When::Random { .. }) => false,
            Trigger::Collision(CollisionWith::Object { name: other_name }) => {
                let other_obj = self.objects.get_obj(other_name)?;

                self.objects[name].poly().collides_with(&other_obj.poly())
            }
            Trigger::Collision(CollisionWith::Area(area)) => {
                let area = c2::AABB::new(c2v(area.min), c2v(area.max));

                self.objects[name].poly().collides_with(&area)
            }
            Trigger::WinStatus(win_status) => match win_status {
                WinStatus::Won => match self.status.current {
                    WinStatus::Won | WinStatus::HasBeenWon => true,
                    _ => false,
                },
                WinStatus::Lost => match self.status.current {
                    WinStatus::Lost | WinStatus::HasBeenLost => true,
                    _ => false,
                },
                WinStatus::NotYetLost => match self.status.current {
                    WinStatus::NotYetLost
                    | WinStatus::NotYetWon
                    | WinStatus::HasBeenWon
                    | WinStatus::Won => true,
                    _ => false,
                },
                WinStatus::NotYetWon => match self.status.current {
                    WinStatus::NotYetWon
                    | WinStatus::NotYetLost
                    | WinStatus::HasBeenLost
                    | WinStatus::Lost => true,
                    _ => false,
                },
                _ => self.status.current == *win_status,
            },
            Trigger::Input(Input::Mouse { over, interaction }) => {
                let is_over = match over {
                    MouseOver::Object { name: other_name } => {
                        let other_obj = self.objects.get_obj(other_name)?;
                        other_obj
                            .poly()
                            .gjk(&c2::Circle::new(c2v(mouse.position), 1.0))
                            .use_radius(false)
                            .run()
                            .distance()
                            == 0.0
                    }
                    MouseOver::Area(area) => is_mouse_in_area(mouse, *area),
                    MouseOver::Anywhere => true,
                };
                is_over
                    && match interaction {
                        MouseInteraction::Button { state } => *state == mouse.state,
                        MouseInteraction::Hover => true,
                    }
            }
            Trigger::CheckProperty {
                name: object_name,
                check,
            } => {
                let obj = self.objects.get_obj(object_name)?;
                match check {
                    PropertyCheck::Switch(switch_state) => obj.switch == *switch_state,
                    PropertyCheck::Sprite(sprite) => obj.sprite == *sprite,
                    PropertyCheck::FinishedAnimation => match obj.animation {
                        AnimationStatus::Finished => true,
                        _ => false,
                    },
                    PropertyCheck::Timer => match obj.timer {
                        Some(alarm) => alarm == 0,
                        None => false,
                    },
                }
            }
            Trigger::Random { chance } => {
                let roll = rand::gen_range::<f32>(0.0, 1.0);
                roll < *chance
            }
            Trigger::DifficultyLevel { level } => self.difficulty == *level,
        };
        Ok(triggered)
    }

    fn check_triggers(&self, name: &str, mouse: &Mouse) -> WeeResult<Vec<Action>> {
        let mut actions = Vec::new();
        for instruction in self.objects[name].instructions.iter() {
            let mut triggered = true;
            for trigger in &instruction.triggers {
                triggered = triggered && self.is_triggered(name, trigger, mouse)?;
            }
            if triggered {
                actions.extend(instruction.actions.clone());
            }
        }
        Ok(actions)
    }

    fn apply_actions(
        &mut self,
        name: &str,
        actions: &[Action],
        mouse: &Mouse,
    ) -> WeeResult<Vec<String>> {
        let mut played_sounds = Vec::new();
        for action in actions {
            self.apply_action(name, action, mouse, &mut played_sounds)?;
        }
        Ok(played_sounds)
    }

    fn apply_action(
        &mut self,
        name: &str,
        action: &Action,
        mouse: &Mouse,
        played_sounds: &mut Vec<String>,
    ) -> WeeResult<()> {
        let try_to_set_status = |status: &mut GameStatus, opposite, next_frame| {
            *status = match status.current {
                WinStatus::NotYetWon | WinStatus::NotYetLost => {
                    if status.next_frame == opposite {
                        *status
                    } else {
                        GameStatus {
                            current: status.current,
                            next_frame,
                        }
                    }
                }
                _ => *status,
            };
        };
        let try_to_win = |status| {
            try_to_set_status(status, WinStatus::HasBeenLost, WinStatus::HasBeenWon);
        };
        let try_to_lose = |status| {
            try_to_set_status(status, WinStatus::HasBeenWon, WinStatus::HasBeenLost);
        };
        match action {
            Action::Motion(motion) => {
                self.objects[name].queued_motion.push(motion.clone());
            }
            Action::Win => {
                try_to_win(&mut self.status);
            }
            Action::Lose => {
                try_to_lose(&mut self.status);
            }
            Action::Effect(new_effect) => {
                self.effect = *new_effect;
            }
            Action::PlaySound { name: sound_name } => {
                played_sounds.push(sound_name.clone());
            }
            Action::StopMusic => {
                // TODO:
                self.has_music_finished = true;
                //self.assets.music.stop();
            }
            Action::Animate {
                animation_type,
                sprites,
                speed,
            } => {
                self.objects[name].animation =
                    AnimationStatus::start(*animation_type, sprites, *speed);

                if let Some(sprite) = sprites.get(0).cloned() {
                    self.objects[name].sprite = sprite;
                }
            }
            Action::DrawText {
                text,
                font,
                colour,
                resize: _resize,
                justify,
            } => {
                self.drawn_text.insert(
                    name.to_string(),
                    DrawnText {
                        text: text.to_string(),
                        font: font.to_string(),
                        colour: *colour,
                        //resize: *resize,
                        justify: *justify,
                    },
                );
            }
            Action::SetProperty(PropertySetter::Angle(angle_setter)) => {
                self.objects[name].angle = match angle_setter {
                    AngleSetter::Value(value) => *value,
                    AngleSetter::Increase(value) => self.objects[name].angle + value,
                    AngleSetter::Decrease(value) => self.objects[name].angle - value,
                    AngleSetter::Match { name: other_name } => {
                        self.objects.get_obj(other_name)?.angle
                    }
                    AngleSetter::Clamp { min, max } => {
                        let mut angle = self.objects[name].angle;
                        if angle < 0.0 {
                            angle += 360.0;
                        }

                        fn clamp_degrees(angle: f32, min: f32, max: f32) -> f32 {
                            fn is_between_angles(angle: f32, min: f32, max: f32) -> bool {
                                if min < max {
                                    angle >= min && angle <= max
                                } else {
                                    angle >= min && angle <= (max + 360.0)
                                        || angle >= (min - 360.0) && angle <= max
                                }
                            }
                            fn distance_between_angles(a: f32, b: f32) -> f32 {
                                let dist1 = (a - b).abs();
                                let dist2 = ((a + 360.0) - b).abs();
                                let dist3 = (a - (b + 360.0)).abs();
                                dist1.min(dist2.min(dist3))
                            }

                            if is_between_angles(angle, min, max) {
                                angle
                            } else if distance_between_angles(angle, min)
                                < distance_between_angles(angle, max)
                            {
                                min
                            } else {
                                max
                            }
                        }
                        clamp_degrees(angle, *min, *max)
                    }
                    AngleSetter::RotateToObject { name: other_name } => {
                        let other_centre = self.objects.get_obj(other_name)?.position;
                        let centre = self.objects[name].origin_in_world();
                        (other_centre.y - centre.y)
                            .atan2(other_centre.x - centre.x)
                            .to_degrees()
                            + 90.0
                    }
                    AngleSetter::RotateToMouse => {
                        let centre = self.objects[name].origin_in_world();
                        let error = 0.00001;
                        if (centre.x - mouse.position.x).abs() < error
                            && (centre.y - mouse.position.y).abs() < error
                        {
                            self.objects[name].angle
                        } else {
                            (mouse.position.y - centre.y)
                                .atan2(mouse.position.x - centre.x)
                                .to_degrees()
                                + 90.0
                        }
                    }
                };
            }
            Action::SetProperty(PropertySetter::Sprite(sprite)) => {
                self.objects[name].sprite = sprite.clone();
                self.objects[name].animation = AnimationStatus::None;
            }
            Action::SetProperty(PropertySetter::Size(size_setter)) => {
                let old_size = self.objects[name].size;
                self.objects[name].size = match size_setter {
                    SizeSetter::Value(value) => *value,
                    SizeSetter::Grow(SizeDifference::Value(value)) => Size {
                        width: self.objects[name].size.width + value.width,
                        height: self.objects[name].size.height + value.height,
                    },
                    SizeSetter::Shrink(SizeDifference::Value(value)) => Size {
                        width: self.objects[name].size.width - value.width,
                        height: self.objects[name].size.height - value.height,
                    },
                    SizeSetter::Grow(SizeDifference::Percent(percent)) => {
                        let w = self.objects[name].size.width;
                        let h = self.objects[name].size.height;
                        Size {
                            width: w + w * (percent.width / 100.0),
                            height: h + h * (percent.height / 100.0),
                        }
                    }
                    SizeSetter::Shrink(SizeDifference::Percent(percent)) => {
                        let w = self.objects[name].size.width;
                        let h = self.objects[name].size.height;
                        Size {
                            width: w - w * (percent.width / 100.0),
                            height: h - h * (percent.height / 100.0),
                        }
                    }
                    SizeSetter::Clamp { min, max } => Size {
                        width: self.objects[name].size.width.min(max.width).max(min.width),
                        height: self.objects[name]
                            .size
                            .height
                            .min(max.height)
                            .max(min.height),
                    },
                };
                let size = self.objects[name].size;
                let difference =
                    Size::new(size.width / old_size.width, size.height / old_size.height);
                match &mut self.objects[name].collision_area {
                    Some(area) => {
                        *area = AABB {
                            min: Vec2::new(
                                area.min.x * difference.width,
                                area.min.y * difference.height,
                            ),
                            max: Vec2::new(
                                area.max.x * difference.width,
                                area.max.y * difference.width,
                            ),
                        };
                    }
                    None => {}
                }
            }
            Action::SetProperty(PropertySetter::Switch(switch)) => {
                if *switch == Switch::On && self.objects[name].switch != SwitchState::On {
                    self.objects[name].switch = SwitchState::SwitchedOn;
                } else if *switch == Switch::Off && self.objects[name].switch != SwitchState::Off {
                    self.objects[name].switch = SwitchState::SwitchedOff;
                }
            }
            Action::SetProperty(PropertySetter::Timer { time }) => {
                self.objects[name].timer = Some(*time);
            }
            Action::SetProperty(PropertySetter::FlipHorizontal(FlipSetter::Flip)) => {
                self.objects[name].flip.horizontal = !self.objects[name].flip.horizontal;
            }
            Action::SetProperty(PropertySetter::FlipVertical(FlipSetter::Flip)) => {
                self.objects[name].flip.vertical = !self.objects[name].flip.vertical;
            }
            Action::SetProperty(PropertySetter::FlipHorizontal(FlipSetter::SetFlip(flipped))) => {
                self.objects[name].flip.horizontal = *flipped;
            }
            Action::SetProperty(PropertySetter::FlipVertical(FlipSetter::SetFlip(flipped))) => {
                self.objects[name].flip.vertical = *flipped;
            }
            Action::SetProperty(PropertySetter::Layer(layer_setter)) => {
                self.objects[name].layer = match layer_setter {
                    LayerSetter::Value(value) => *value,
                    LayerSetter::Increase => {
                        self.objects[name].layer
                            + if self.objects[name].layer < std::u8::MAX - 1 {
                                1
                            } else {
                                0
                            }
                    }
                    LayerSetter::Decrease => {
                        self.objects[name].layer - if self.objects[name].layer > 0 { 1 } else { 0 }
                    }
                };
            }
            Action::Random { random_actions } => {
                let action = random_actions.choose();
                if let Some(action) = action {
                    return self.apply_action(name, &action, mouse, played_sounds);
                }
            }
            Action::EndEarly => {
                self.end_early = true;
            }
        };

        Ok(())
    }

    fn move_object(&mut self, name: &str, mouse: &Mouse) -> WeeResult<()> {
        let mut clamps = Vec::new();
        for mut motion in self.objects[name].queued_motion.clone().into_iter() {
            if let Motion::JumpTo(JumpLocation::ClampPosition { .. }) = &motion {
            } else {
                for area in clamps {
                    clamp_position(&mut self.objects[name].position, area);
                }
                clamps = Vec::new();
            }
            self.objects[name].active_motion = match &mut motion {
                Motion::GoStraight { direction, speed } => {
                    let velocity = direction.to_vector(&self.objects[name], *speed);
                    ActiveMotion::GoStraight { velocity }
                }
                Motion::JumpTo(jump_location) => {
                    match jump_location {
                        JumpLocation::Point(point) => {
                            self.objects[name].position = *point;
                        }
                        JumpLocation::Relative { to, distance } => match to {
                            RelativeTo::CurrentPosition => {
                                self.objects[name].position += *distance;
                            }
                            RelativeTo::CurrentAngle => {
                                let angle = self.objects[name].trig_angle();
                                self.objects[name].position.x +=
                                    -distance.y * angle.cos() - distance.x * angle.sin();
                                self.objects[name].position.y +=
                                    -distance.y * angle.sin() + distance.x * angle.cos();
                            }
                        },
                        JumpLocation::Area(area) => {
                            fn gen_in_area(area: AABB) -> Vec2 {
                                Vec2::new(
                                    gen_in_range(area.min.x, area.max.x),
                                    gen_in_range(area.min.y, area.max.y),
                                )
                            }
                            self.objects[name].position = gen_in_area(*area);
                        }
                        JumpLocation::ClampPosition { .. } => {
                            //clamp_position(&mut self.objects[name].position, *area);
                        }
                        JumpLocation::Object { name: other_name } => {
                            self.objects[name].position =
                                self.objects.get_obj(&other_name)?.position;
                        }
                        JumpLocation::Mouse => {
                            self.objects[name].position = mouse.position;
                        }
                    }
                    if let Motion::JumpTo(JumpLocation::ClampPosition { area }) = motion {
                        clamps.push(area);
                        self.objects[name].active_motion.clone()
                    } else {
                        ActiveMotion::Stop
                    }
                }
                Motion::Roam {
                    movement_type,
                    area,
                    speed,
                } => {
                    let active_roam = match movement_type {
                        MovementType::Wiggle => ActiveRoam::Wiggle,
                        MovementType::Reflect {
                            initial_direction,
                            movement_handling,
                        } => {
                            let width = area.width();
                            let height = area.height();
                            match initial_direction {
                                MovementDirection::Angle(angle) => {
                                    let angle = match angle {
                                        Angle::Current => self.objects[name].angle,
                                        Angle::Degrees(degrees) => *degrees,
                                        Angle::Random { min, max } => gen_in_range(*min, *max),
                                    };
                                    let velocity = vector_from_angle(angle, *speed);
                                    ActiveRoam::Reflect {
                                        velocity,
                                        movement_handling: *movement_handling,
                                    }
                                }
                                MovementDirection::Direction {
                                    possible_directions,
                                } => {
                                    let enough_horizontal_space =
                                        width < self.objects[name].size.width;
                                    let enough_vertical_space =
                                        height < self.objects[name].size.height;

                                    let possible_directions = if !possible_directions.is_empty() {
                                        possible_directions.iter().cloned().collect()
                                    } else if enough_horizontal_space && enough_vertical_space {
                                        Vec::new()
                                    } else if enough_horizontal_space {
                                        vec![CompassDirection::Up, CompassDirection::Down]
                                    } else if enough_vertical_space {
                                        vec![CompassDirection::Left, CompassDirection::Right]
                                    } else {
                                        CompassDirection::all_directions()
                                    };
                                    let dir = possible_directions.choose();
                                    let velocity = match dir {
                                        Some(dir) => dir.to_vector(*speed),
                                        None => Vec2::zero(),
                                    };
                                    ActiveRoam::Reflect {
                                        velocity,
                                        movement_handling: *movement_handling,
                                    }
                                }
                            }
                        }
                        MovementType::Insect => ActiveRoam::Insect {
                            velocity: random_velocity(*speed),
                        },
                        MovementType::Bounce { initial_direction } => {
                            let frames_in_bounce =
                                60.0 * Speed::Normal.as_value() / speed.as_value();
                            let acceleration = -2.0 * (area.min.y - area.max.y)
                                / (frames_in_bounce * frames_in_bounce);
                            let velocity = {
                                let y_velocity = 2.0 * (area.min.y - self.objects[name].position.y)
                                    / frames_in_bounce;
                                Vec2::new(0.0, y_velocity)
                            };
                            let direction = initial_direction.clone().unwrap_or_else(|| {
                                if rand::gen_range(0, 2) == 0 {
                                    BounceDirection::Left
                                } else {
                                    BounceDirection::Right
                                }
                            });

                            ActiveRoam::Bounce {
                                velocity,
                                direction,
                                acceleration,
                                frames_in_bounce,
                            }
                        }
                    };
                    ActiveMotion::Roam {
                        movement_type: active_roam,
                        area: *area,
                        speed: *speed,
                    }
                }
                Motion::Swap { name: other_name } => {
                    let other_name = &*other_name;
                    self.objects.get_obj(&other_name)?;
                    let temp = self.objects[other_name].position;
                    self.objects[other_name].position = self.objects[name].position;
                    self.objects[name].position = temp;
                    ActiveMotion::Stop
                }
                Motion::Target {
                    target,
                    target_type,
                    offset,
                    speed,
                } => ActiveMotion::Target {
                    target: target.clone(),
                    target_type: *target_type,
                    offset: *offset,
                    speed: *speed,
                },
                Motion::Accelerate(Acceleration::Continuous { direction, speed }) => {
                    let speed = Speed::Value(speed.as_value() / 40.0);
                    let acceleration = direction.to_vector(&self.objects[name], speed);
                    let velocity = match &self.objects[name].active_motion {
                        ActiveMotion::Accelerate { velocity, .. } => *velocity,
                        ActiveMotion::GoStraight { velocity } => *velocity,
                        ActiveMotion::Roam { movement_type, .. } => match movement_type {
                            ActiveRoam::Insect { velocity } => *velocity,
                            ActiveRoam::Bounce { velocity, .. } => *velocity,
                            ActiveRoam::Reflect { velocity, .. } => *velocity,
                            _ => Vec2::zero(),
                        },
                        ActiveMotion::Target { .. } => Vec2::zero(),
                        ActiveMotion::SlowDown { velocity, .. } => *velocity,
                        ActiveMotion::Stop => Vec2::zero(),
                    };
                    ActiveMotion::Accelerate {
                        velocity,
                        acceleration,
                    }
                }
                Motion::Accelerate(Acceleration::SlowDown { speed }) => {
                    let velocity = match &self.objects[name].active_motion {
                        ActiveMotion::Accelerate { velocity, .. } => *velocity,
                        ActiveMotion::GoStraight { velocity } => *velocity,
                        ActiveMotion::Roam { movement_type, .. } => match movement_type {
                            ActiveRoam::Insect { velocity } => *velocity,
                            ActiveRoam::Bounce { velocity, .. } => *velocity,
                            ActiveRoam::Reflect { velocity, .. } => *velocity,
                            _ => Vec2::zero(),
                        },
                        ActiveMotion::Target { .. } => Vec2::zero(),
                        ActiveMotion::SlowDown { velocity, .. } => *velocity,
                        ActiveMotion::Stop => Vec2::zero(),
                    };
                    if velocity.x == 0.0 && velocity.y == 0.0 {
                        ActiveMotion::Stop
                    } else {
                        let deceleration = -(velocity.unit() * (speed.as_value() / 40.0));
                        ActiveMotion::SlowDown {
                            velocity,
                            deceleration,
                        }
                    }
                }
                Motion::Stop => ActiveMotion::Stop,
            };
        }
        self.objects[name].queued_motion = Vec::new();

        self.objects[name].active_motion = self.update_active_motion(name, mouse)?;

        for area in clamps {
            clamp_position(&mut self.objects[name].position, area);
            self.objects[name].active_motion = ActiveMotion::Stop;
        }

        Ok(())
    }

    fn update_active_motion(&mut self, name: &str, mouse: &Mouse) -> WeeResult<ActiveMotion> {
        let active_motion = match self.objects[name].active_motion.clone() {
            ActiveMotion::GoStraight { velocity } => {
                self.objects[name].position += velocity;
                ActiveMotion::GoStraight { velocity }
            }
            ActiveMotion::Roam {
                movement_type,
                area,
                speed,
            } => {
                let movement_type = match movement_type {
                    ActiveRoam::Wiggle => {
                        self.objects[name].position += random_velocity(speed);
                        clamp_position(&mut self.objects[name].position, area);

                        ActiveRoam::Wiggle
                    }
                    ActiveRoam::Insect { mut velocity } => {
                        const CHANGE_DIRECTION_PROBABILTY: f32 = 0.1;
                        if rand::gen_range::<f32>(0.0, 1.0) < CHANGE_DIRECTION_PROBABILTY {
                            velocity = random_velocity(speed);
                        }
                        self.objects[name].position += velocity;

                        clamp_position(&mut self.objects[name].position, area);

                        ActiveRoam::Insect { velocity }
                    }
                    ActiveRoam::Reflect {
                        mut velocity,
                        movement_handling,
                    } => {
                        if let MovementHandling::TryNotToOverlap = movement_handling {
                            fn calculate_closest_manifold<T: BasicShape>(
                                objects: &Objects,
                                name: &str,
                                poly: T,
                            ) -> (Option<c2::Manifold>, Vec2) {
                                let mut longest_depth = 0.0;
                                let mut closest_manifold = None;
                                let mut position = Vec2::zero();
                                for other_name in objects.keys() {
                                    if other_name != name {
                                        let manifold = poly.manifold(&objects[other_name].poly());
                                        if manifold.count() > 0 {
                                            let depth = manifold.depths()[0];
                                            if depth > longest_depth || closest_manifold.is_none() {
                                                closest_manifold = Some(manifold);
                                                position = objects[other_name].position;
                                                longest_depth = depth;
                                            }
                                        }
                                    }
                                }
                                (closest_manifold, position)
                            }
                            let (original_manifold, other_position) = calculate_closest_manifold(
                                &self.objects,
                                name,
                                self.objects[name].poly(),
                            );
                            let move_away = |manifold: Option<c2::Manifold>| {
                                if let Some(manifold) = manifold {
                                    let normal = manifold.normal();
                                    let depth = manifold.depths()[0];
                                    Vec2::new(normal.x() * depth, normal.y() * depth)
                                } else {
                                    Vec2::zero()
                                }
                            };
                            self.objects[name].position -= move_away(original_manifold);

                            let closest_manifold = {
                                let moved_poly = {
                                    let transformation = c2::Transformation::new(
                                        [velocity.x, velocity.y],
                                        c2::Rotation::zero(),
                                    );
                                    (self.objects[name].poly(), transformation)
                                };

                                let (new_manifold, _) =
                                    calculate_closest_manifold(&self.objects, name, moved_poly);

                                let is_moving_towards = {
                                    let to_point = other_position - self.objects[name].position;
                                    (to_point.x * velocity.x + to_point.y * velocity.y) > 0.0
                                };
                                if new_manifold.is_none() && is_moving_towards {
                                    original_manifold
                                } else {
                                    new_manifold
                                }
                            };
                            self.objects[name].position -= move_away(closest_manifold);
                            if let Some(manifold) = closest_manifold {
                                let normal = manifold.normal();
                                let len = (normal.x().powf(2.0) + normal.y().powf(2.0)).sqrt();
                                if len != 0.0 {
                                    let normal = Vec2::new(normal.x() / len, normal.y() / len);
                                    velocity = velocity - 2.0 * (velocity * normal) * normal;
                                }
                            }
                        }
                        if self.objects[name].position.x + velocity.x < area.min.x {
                            velocity.x = velocity.x.abs();
                        }
                        if self.objects[name].position.x + velocity.x > area.max.x {
                            velocity.x = -velocity.x.abs();
                        }
                        if self.objects[name].position.y + velocity.y < area.min.y {
                            velocity.y = velocity.y.abs();
                        }
                        if self.objects[name].position.y + velocity.y > area.max.y {
                            velocity.y = -velocity.y.abs();
                        }
                        self.objects[name].position += velocity;

                        ActiveRoam::Reflect {
                            velocity,
                            movement_handling,
                        }
                    }
                    ActiveRoam::Bounce {
                        mut velocity,
                        mut direction,
                        acceleration,
                        frames_in_bounce,
                    } => {
                        let (x, y) = (self.objects[name].position.x, self.objects[name].position.y);
                        if y < area.min.y && velocity.y < 0.0 {
                            velocity.y = 0.0;
                        }
                        if y < area.max.y {
                            velocity.y += acceleration;
                        } else if y > area.max.y {
                            velocity.y = -acceleration * frames_in_bounce;
                        }
                        if x > area.max.x {
                            direction = BounceDirection::Left;
                        } else if x < area.min.x {
                            direction = BounceDirection::Right;
                        }
                        if x >= area.min.x
                            && x <= area.max.x
                            && self.objects[name].size.width >= area.width()
                        {
                            velocity.x = 0.0;
                        } else {
                            match direction {
                                BounceDirection::Left => velocity.x = -speed.as_value() / 2.0,
                                BounceDirection::Right => velocity.x = speed.as_value() / 2.0,
                            }
                        }
                        self.objects[name].position += velocity;

                        ActiveRoam::Bounce {
                            velocity,
                            direction,
                            acceleration,
                            frames_in_bounce,
                        }
                    }
                };
                ActiveMotion::Roam {
                    movement_type,
                    area,
                    speed,
                }
            }
            ActiveMotion::Target {
                target,
                target_type,
                offset,
                speed,
            } => {
                self.objects[name].position = {
                    let other = match &target {
                        Target::Object { name: other_name } => {
                            self.objects.get_obj(other_name)?.position
                        }
                        Target::Mouse => mouse.position,
                    };
                    let target_vector = other + offset - self.objects[name].position;
                    let target_vector = target_vector
                        / (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                    let move_to = |x: f32, other: f32, velocity: f32| {
                        if (x - other).abs() > velocity.abs() {
                            x + velocity
                        } else {
                            other
                        }
                    };
                    let velocity: Vec2 = target_vector * speed.as_value();

                    Vec2::new(
                        move_to(
                            self.objects[name].position.x,
                            other.x + offset.x,
                            velocity.x,
                        ),
                        move_to(
                            self.objects[name].position.y,
                            other.y + offset.y,
                            velocity.y,
                        ),
                    )
                };

                if let TargetType::StopWhenReached = target_type {
                    let other = match &target {
                        Target::Object { name: other_name } => {
                            self.objects.get_obj(other_name)?.position
                        }
                        Target::Mouse => mouse.position,
                    };
                    let close_enough =
                        |pos: f32, other: f32, offset: f32| (pos - (other + offset)).abs() < 0.5;
                    if close_enough(self.objects[name].position.x, other.x, offset.x)
                        && close_enough(self.objects[name].position.y, other.y, offset.y)
                    {
                        ActiveMotion::Stop
                    } else {
                        ActiveMotion::Target {
                            target,
                            target_type,
                            offset,
                            speed,
                        }
                    }
                } else {
                    ActiveMotion::Target {
                        target,
                        target_type,
                        offset,
                        speed,
                    }
                }
            }
            ActiveMotion::Accelerate {
                mut velocity,
                acceleration,
            } => {
                self.objects[name].position += velocity;
                velocity += acceleration;
                ActiveMotion::Accelerate {
                    velocity,
                    acceleration,
                }
            }
            ActiveMotion::SlowDown {
                mut velocity,
                deceleration,
            } => {
                if velocity.magnitude() <= deceleration.magnitude() {
                    ActiveMotion::Stop
                } else {
                    self.objects[name].position += velocity;
                    velocity += deceleration;
                    ActiveMotion::SlowDown {
                        velocity,
                        deceleration,
                    }
                }
            }
            ActiveMotion::Stop => ActiveMotion::Stop,
        };

        Ok(active_motion)
    }
}
