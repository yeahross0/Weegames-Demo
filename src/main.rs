use macroquad::logging as log;
use macroquad::prelude::*;
use macroquad::texture;
use macroquad::{
    audio::{self, PlaySoundParams, Sound},
    experimental::coroutines::{start_coroutine, Coroutine},
};

use futures::future::join_all;
use std::{
    collections::{HashMap, HashSet},
    default::Default,
    path::Path,
    str,
};

mod wee;

use wee::*;

const PROJECTION_WIDTH: f32 = 1600.0;
const PROJECTION_HEIGHT: f32 = 900.0;

const DEFAULT_DIFFICULTY: u32 = 1;
const DEFAULT_PLAYBACK_RATE: f32 = 1.0;
const MAX_LIVES: i32 = 4;
const INITIAL_PLAYBACK_RATE: f32 = 1.0;
const PLAYBACK_RATE_INCREASE: f32 = 0.1;
const PLAYBACK_RATE_MAX: f32 = 2.0;
const UP_TO_DIFFICULTY_TWO: i32 = 20;
const UP_TO_DIFFICULTY_THREE: i32 = 40;
const BOSS_GAME_INTERVAL: i32 = 15;
const INCREASE_SPEED_AFTER_GAMES: i32 = 5;
const VOLUME: f32 = 0.5;

async fn load_images<P: AsRef<Path>>(
    image_files: &HashMap<String, String>,
    base_path: P,
) -> WeeResult<Images> {
    log::debug!("Start loading images");
    let mut paths = Vec::new();
    let mut images = Images::new();
    let mut loading_images = Vec::new();

    let base_path = base_path.as_ref().join("images");

    for path in image_files.values() {
        let path = base_path.join(path);

        let path = path.to_str().unwrap().to_string();
        paths.push(path);
    }

    for path in &paths {
        loading_images.push(texture::load_texture(path));
    }

    let textures: Vec<_> = join_all(loading_images).await;

    for (key, texture) in image_files.keys().zip(textures) {
        let texture = texture?;
        texture.set_filter(macroquad::texture::FilterMode::Nearest);
        images.insert(key.to_string(), texture);
    }

    Ok(images)
}

async fn load_sounds(
    sound_files: &HashMap<String, String>,
    base_path: impl AsRef<Path>,
) -> WeeResult<Sounds> {
    let base_path = base_path.as_ref().join("audio");
    let mut sounds = Sounds::new();

    for (key, filename) in sound_files {
        let path = base_path.join(&filename);

        let sound = macroquad::audio::load_sound(&path.to_str().unwrap()).await?;

        sounds.insert(key.to_string(), sound);
    }
    Ok(sounds)
}

#[derive(Clone)]
struct Music {
    data: Sound,
    looped: bool,
}

async fn load_music(
    music_file: &Option<SerialiseMusic>,
    base_path: impl AsRef<Path>,
) -> WeeResult<Option<Music>> {
    let base_path = base_path.as_ref().join("audio");

    if let Some(music_info) = music_file {
        let path = base_path.join(&music_info.filename);

        let sound = macroquad::audio::load_sound(&path.to_str().unwrap()).await?;

        Ok(Some(Music {
            data: sound,
            looped: music_info.looped,
        }))
    } else {
        Ok(None)
    }
}

pub trait MusicPlayer {
    fn play(&self, playback_rate: f32, volume: f32);

    fn stop(&self);
}

impl MusicPlayer for Option<Music> {
    fn play(&self, playback_rate: f32, volume: f32) {
        if let Some(music) = self {
            macroquad::audio::play_sound(
                music.data,
                PlaySoundParams {
                    looped: music.looped,
                    volume: volume,
                    speed: playback_rate,
                },
            );
        }
    }

    fn stop(&self) {
        if let Some(music) = self {
            macroquad::audio::stop_sound(music.data);
        }
    }
}

impl Drop for Music {
    fn drop(&mut self) {
        macroquad::audio::stop_sound(self.data);
    }
}

async fn load_fonts(
    font_files: &HashMap<String, FontLoadInfo>,
    base_path: impl AsRef<Path>,
) -> WeeResult<Fonts> {
    let base_path = base_path.as_ref().join("fonts");
    let mut fonts = Fonts::new();

    for (key, font_info) in font_files {
        let path = base_path.join(&font_info.filename);

        let font = macroquad::text::load_ttf_font(path.to_str().unwrap()).await?;
        fonts.insert(key.to_string(), (font, font_info.size as u16));
    }
    Ok(fonts)
}

type Images = HashMap<String, Texture2D>;
type Fonts = HashMap<String, (Font, u16)>;
type Sounds = HashMap<String, Sound>;

#[derive(Clone)]
struct LoadedGameData {
    data: GameData,
    images: Images,
    music: Option<Music>,
    sounds: Sounds,
    fonts: Fonts,
}

impl LoadedGameData {
    async fn load(filename: impl AsRef<Path>) -> WeeResult<LoadedGameData> {
        let game_data = GameData::load(&filename).await.unwrap();
        let base_path = filename.as_ref().parent().unwrap();
        let data = LoadedGameData {
            images: load_images(&game_data.asset_files.images, base_path).await?,
            music: load_music(&game_data.asset_files.music, base_path).await?,
            sounds: load_sounds(&game_data.asset_files.audio, base_path).await?,
            fonts: load_fonts(&game_data.asset_files.fonts, base_path).await?,
            data: game_data,
        };
        Ok(data)
    }
}

#[derive(Clone)]
struct Assets {
    images: Images,
    music: Option<Music>,
    sounds: Sounds,
    fonts: Fonts,
}

impl Assets {
    async fn load(asset_files: &AssetFiles, base_path: impl AsRef<Path>) -> WeeResult<Assets> {
        let assets = Assets {
            images: load_images(&asset_files.images, &base_path).await?,
            music: load_music(&asset_files.music, &base_path).await?,
            sounds: load_sounds(&asset_files.audio, &base_path).await?,
            fonts: load_fonts(&asset_files.fonts, &base_path).await?,
        };
        Ok(assets)
    }

    fn stop_sounds(&self) {
        self.music.stop();

        for sound in self.sounds.values() {
            audio::stop_sound(*sound);
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct LastGame {
    has_won: bool,
    was_life_gained: bool,
}

#[derive(Debug, Copy, Clone)]
struct Progress {
    playback_rate: f32,
    score: i32,
    lives: i32,
    difficulty: u32,
    last_game: Option<LastGame>,
    boss_playback_rate: f32,
}

impl Progress {
    fn new() -> Progress {
        Progress {
            playback_rate: INITIAL_PLAYBACK_RATE,
            score: 0,
            lives: MAX_LIVES,
            difficulty: DEFAULT_DIFFICULTY,
            last_game: None,
            boss_playback_rate: INITIAL_PLAYBACK_RATE,
        }
    }

    fn update(&mut self, has_won: bool, is_boss_game: bool) {
        self.score += 1;
        if self.score % INCREASE_SPEED_AFTER_GAMES == 0 {
            self.playback_rate += PLAYBACK_RATE_INCREASE;
        }
        if self.score >= UP_TO_DIFFICULTY_THREE {
            self.difficulty = 3;
        } else if self.score >= UP_TO_DIFFICULTY_TWO {
            self.difficulty = 2;
        }
        self.playback_rate = self.playback_rate.min(PLAYBACK_RATE_MAX);

        if is_boss_game {
            self.boss_playback_rate += PLAYBACK_RATE_INCREASE;
        }

        if !has_won {
            self.lives -= 1;
        }

        let was_life_gained = is_boss_game && has_won && self.lives < MAX_LIVES;
        if was_life_gained {
            self.lives += 1;
        }
        self.last_game = Some(LastGame {
            has_won,
            was_life_gained,
        });
    }
}

// Adapted from draw_rectangle/draw_texture_ex in macroquad
fn draw_rectangle_ex(
    color: Color,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rotation: f32,
    pivot: Option<glam::f32::Vec2>,
) {
    unsafe {
        let gl = macroquad::window::get_internal_gl().quad_gl;

        let pivot = pivot.unwrap_or(vec2(x + w / 2., y + h / 2.));
        let m = pivot;
        let p = [
            vec2(x, y) - pivot,
            vec2(x + w, y) - pivot,
            vec2(x + w, y + h) - pivot,
            vec2(x, y + h) - pivot,
        ];
        let r = rotation;
        let p = [
            vec2(
                p[0].x * r.cos() - p[0].y * r.sin(),
                p[0].x * r.sin() + p[0].y * r.cos(),
            ) + m,
            vec2(
                p[1].x * r.cos() - p[1].y * r.sin(),
                p[1].x * r.sin() + p[1].y * r.cos(),
            ) + m,
            vec2(
                p[2].x * r.cos() - p[2].y * r.sin(),
                p[2].x * r.sin() + p[2].y * r.cos(),
            ) + m,
            vec2(
                p[3].x * r.cos() - p[3].y * r.sin(),
                p[3].x * r.sin() + p[3].y * r.cos(),
            ) + m,
        ];
        #[rustfmt::skip]
        let vertices = [
            Vertex::new(p[0].x, p[0].y, 0.0,  0.0,  0.0, color),
            Vertex::new(p[1].x, p[1].y, 0.0, 1.0,  0.0, color),
            Vertex::new(p[2].x, p[2].y, 0.0, 1.0, 1.0, color),
            Vertex::new(p[3].x, p[3].y, 0.0,  0.0, 1.0, color),
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        gl.texture(None);
        gl.draw_mode(DrawMode::Triangles);
        gl.geometry(&vertices, &indices);
    }
}

fn draw_game(game: &Game, images: &Images, fonts: &Fonts, intro_font: &Font) {
    // Draw background
    for part in &game.background {
        match &part.sprite {
            Sprite::Image { name } => {
                let params = macroquad::texture::DrawTextureParams {
                    dest_size: Some(macroquad::math::Vec2::new(
                        part.area.width(),
                        part.area.height(),
                    )),
                    source: None,
                    rotation: 0.0,
                    pivot: None,
                    flip_x: false,
                    flip_y: false,
                };
                draw_texture_ex(
                    images[name],
                    part.area.min.x,
                    part.area.min.y,
                    macroquad::color::WHITE,
                    params,
                );
            }
            Sprite::Colour(colour) => macroquad::shapes::draw_rectangle(
                part.area.min.x,
                part.area.min.y,
                part.area.max.x,
                part.area.max.y,
                macroquad::color::Color::new(colour.r, colour.g, colour.b, colour.a),
            ),
        }
    }

    // Draw Objects
    let mut layers: Vec<u8> = game.objects.values().map(|o| o.layer).collect();
    layers.sort();
    layers.dedup();
    layers.reverse();
    for layer in layers.into_iter() {
        for (key, object) in game.objects.iter() {
            if object.layer == layer {
                match &object.sprite {
                    Sprite::Image { name } => {
                        let origin = object.origin_in_world();
                        let origin = macroquad::math::Vec2::new(origin.x, origin.y);
                        let params = macroquad::texture::DrawTextureParams {
                            dest_size: Some(macroquad::math::Vec2::new(
                                object.size.width,
                                object.size.height,
                            )),
                            source: None,
                            rotation: object.angle.to_radians(),
                            pivot: Some(origin),
                            flip_x: object.flip.horizontal,
                            flip_y: object.flip.vertical,
                        };
                        draw_texture_ex(
                            images[name],
                            object.position.x - object.size.width / 2.0,
                            object.position.y - object.size.height / 2.0,
                            macroquad::color::WHITE,
                            params,
                        );
                    }
                    Sprite::Colour(colour) => {
                        let origin = object.origin_in_world();
                        let origin = macroquad::math::Vec2::new(origin.x, origin.y);
                        draw_rectangle_ex(
                            Color::new(colour.r, colour.g, colour.b, colour.a),
                            object.position.x - object.size.width / 2.0,
                            object.position.y - object.size.height / 2.0,
                            object.size.width,
                            object.size.height,
                            object.angle.to_radians(),
                            Some(origin),
                        );
                    }
                }

                if game.drawn_text.contains_key(key) {
                    let colour = game.drawn_text[key].colour;
                    let colour = Color::new(colour.r, colour.g, colour.b, colour.a);
                    let size = macroquad::text::measure_text(
                        &game.drawn_text[key].text,
                        Some(fonts[&game.drawn_text[key].font].0),
                        fonts[&game.drawn_text[key].font].1,
                        1.0,
                    );
                    let position = match game.drawn_text[key].justify {
                        JustifyText::Left => wee::Vec2::new(
                            object.position.x - object.half_width(),
                            object.position.y, // - size.height / 1.25,
                        ),
                        JustifyText::Centre => wee::Vec2::new(
                            object.position.x - size.width / 2.0,
                            object.position.y, // - size.height / 2.0,
                        ),
                    };
                    let params = macroquad::text::TextParams {
                        font: fonts[&game.drawn_text[key].font].0,
                        font_size: fonts[&game.drawn_text[key].font].1,
                        font_scale: 1.0,
                        font_scale_aspect: 1.0,
                        color: colour,
                    };
                    macroquad::text::draw_text_ex(
                        &game.drawn_text[key].text,
                        position.x,
                        position.y,
                        params,
                    );
                }
            }
        }
    }

    // Draw Intro Text
    const INTRO_TEXT_TIME: u32 = 60;
    if game.frames.ran < INTRO_TEXT_TIME {
        let colour = BLACK;
        let size = macroquad::text::measure_text(&game.intro_text, Some(*intro_font), 176, 1.01);
        let params = macroquad::text::TextParams {
            font: *intro_font,
            font_size: 178,
            font_scale: 1.01,
            font_scale_aspect: 1.0,
            color: colour,
        };
        macroquad::text::draw_text_ex(
            &game.intro_text,
            PROJECTION_WIDTH / 2.0 - size.width / 2.0,
            PROJECTION_HEIGHT / 2.0,
            params,
        );

        let colour = WHITE;
        let size = macroquad::text::measure_text(&game.intro_text, Some(*intro_font), 174, 1.0);
        let params = macroquad::text::TextParams {
            font: *intro_font,
            font_size: 174,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            color: colour,
        };
        macroquad::text::draw_text_ex(
            &game.intro_text,
            PROJECTION_WIDTH / 2.0 - size.width / 2.0,
            PROJECTION_HEIGHT / 2.0,
            params,
        );
    }
}

fn update_frame(game: &mut Game, assets: &Assets, playback_rate: f32) -> WeeResult<()> {
    let position = macroquad::input::mouse_position();
    let position = wee::Vec2::new(position.0 as f32, position.1 as f32);
    let position = wee::Vec2::new(
        position.x / macroquad::window::screen_width() as f32 * PROJECTION_WIDTH,
        position.y / macroquad::window::screen_height() as f32 * PROJECTION_HEIGHT,
    );
    let mouse = Mouse {
        position,
        state: if macroquad::input::is_mouse_button_pressed(MouseButton::Left) {
            ButtonState::Press
        } else if macroquad::input::is_mouse_button_released(MouseButton::Left) {
            ButtonState::Release
        } else if macroquad::input::is_mouse_button_down(MouseButton::Left) {
            ButtonState::Down
        } else {
            ButtonState::Up
        },
    };

    let played_sounds = game.update(&mouse)?;

    for played_sound in played_sounds {
        audio::play_sound(
            assets.sounds[&played_sound],
            PlaySoundParams {
                looped: false,
                volume: VOLUME,
                speed: playback_rate,
            },
        );
    }

    if game.has_music_finished {
        assets.music.stop();
    }

    game.status.current = game.status.next_frame;
    game.status.next_frame = match game.status.next_frame {
        WinStatus::HasBeenWon => WinStatus::Won,
        WinStatus::HasBeenLost => WinStatus::Lost,
        _ => game.status.next_frame,
    };
    game.frames.ran += 1;

    Ok(())
}

#[derive(Debug)]
struct GamesList {
    games: Vec<&'static str>,
    bosses: Vec<&'static str>,
    next: Vec<&'static str>,
    directory: String,
}

impl GamesList {
    fn from_directory(all_games: &HashMap<&'static str, GameData>, directory: String) -> GamesList {
        let mut games = Vec::new();
        let mut bosses = Vec::new();
        for (filename, game) in all_games {
            // TODO: Wiser way of doing this
            if Path::new(&filename).starts_with(&directory) && game.published {
                //if filename.starts_with(&directory) && game.published {
                if game.game_type == GameType::Minigame {
                    games.push(*filename);
                } else if game.game_type == GameType::BossGame {
                    bosses.push(*filename);
                }
            }
        }

        GamesList {
            games,
            bosses,
            directory,
            next: Vec::new(),
        }
    }

    fn choose_game(&mut self) -> &'static str {
        while !self.games.is_empty() && self.next.len() < 5 {
            let game = self.games.remove(rand::gen_range(0, self.games.len()));
            self.next.push(game);
        }

        let next = self.next.remove(0);
        self.games.push(next);
        next
    }

    fn choose_boss(&self) -> &'static str {
        self.bosses[rand::gen_range(0, self.bosses.len())]
    }
}

// Adapted from macroquad::storage
mod dispenser {
    use std::any::Any;

    static mut STORAGE: Option<Box<dyn Any>> = None;

    pub fn store<T: Any>(data: T) {
        unsafe {
            STORAGE = Some(Box::new(data));
        };
    }

    pub fn take<T: Any>() -> T {
        unsafe { *STORAGE.take().unwrap().downcast::<T>().unwrap() }
    }
}

fn frames_to_run(frames: FrameInfo, playback_rate: f32) -> u32 {
    let mut num_frames = playback_rate.floor();
    let remainder = playback_rate - num_frames;
    if remainder != 0.0 {
        let how_often_extra = 1.0 / remainder;
        if (frames.steps_taken as f32 % how_often_extra).floor() == 0.0 {
            num_frames += 1.0;
        }
    }
    match frames.remaining() {
        FrameCount::Frames(remaining) => (num_frames as u32).min(remaining),
        FrameCount::Infinite => num_frames as u32,
    }
}

fn preloaded_game<'a>(
    games: &HashMap<&'static str, GameData>,
    preloaded_assets: &'a HashMap<&'static str, Assets>,
    directory: &str,
    filename: &str,
) -> (GameData, &'a Assets) {
    let mode_path = |directory: &str, filename| {
        let mut path = directory.to_string();
        if !path.ends_with('/') {
            path.push_str("/");
        };
        path.push_str(filename);
        path
    };
    let file_path = mode_path(directory, filename);

    let (game, assets) = (
        games.get(file_path.as_str()),
        preloaded_assets.get(file_path.as_str()),
    );

    if let (Some(game), Some(assets)) = (game, assets) {
        (game.clone(), assets)
    } else {
        let file_path = format!("games/system/{}", filename);
        let game = games[file_path.as_str()].clone();
        let assets = &preloaded_assets[file_path.as_str()];
        (game, assets)
    }
}

struct MainGame<S> {
    state: S,
    intro_font: Font,
    games: HashMap<&'static str, GameData>,
    preloaded_assets: HashMap<&'static str, Assets>,
    high_scores: HashMap<String, (i32, i32, i32)>,
    played_games: HashSet<&'static str>,
}

struct LoadingScreen {}

impl MainGame<LoadingScreen> {
    async fn load() -> WeeResult<MainGame<Menu>> {
        let game = LoadedGameData::load("games/system/loading-screen.json").await?;

        let assets = Assets {
            images: game.images,
            fonts: game.fonts,
            sounds: game.sounds,
            music: game.music,
        };

        let mut game = Game::from_data(game.data);

        let intro_font = macroquad::text::load_ttf_font("fonts/Roboto-Medium.ttf").await?;

        let game_filenames = vec![
            "games/second/bike.json",
            "games/second/break.json",
            "games/second/dawn.json",
            "games/second/disc.json",
            "games/second/explosion.json",
            "games/second/gravity.json",
            "games/second/jump.json",
            "games/second/knock.json",
            "games/second/mine.json",
            "games/second/racer.json",
            "games/second/slide.json",
            "games/second/stealth.json",
            "games/second/swim.json",
            "games/second/tanks.json",
            "games/second/time.json",
            "games/yeah/baby.json",
            "games/yeah/balloon.json",
            "games/yeah/bird.json",
            "games/yeah/boss.json",
            "games/yeah/boxer.json",
            "games/yeah/cannon.json",
            "games/yeah/cat.json",
            "games/yeah/disgrace.json",
            "games/yeah/hiding.json",
            "games/yeah/mask.json",
            "games/yeah/monkey.json",
            "games/yeah/orange.json",
            "games/yeah/parachute.json",
            "games/yeah/piano.json",
            "games/yeah/planes.json",
            "games/yeah/pumpkin.json",
            "games/yeah/puzzle.json",
            "games/yeah/quake.json",
            "games/yeah/rhinos.json",
            "games/yeah/shake.json",
            "games/yeah/shed.json",
            "games/yeah/titanic.json",
            "games/yeah/wasp.json",
            "games/second/prelude.json",
            "games/system/prelude.json",
            "games/second/interlude.json",
            "games/system/interlude.json",
            "games/second/boss.json",
            "games/second/game-over.json",
            "games/system/game-over.json",
            "games/system/choose-mode.json",
            "games/mine/bong.json",
            "games/bops/cloud.json",
        ];

        let games_to_preload = vec![
            "games/second/prelude.json",
            "games/system/prelude.json",
            "games/second/interlude.json",
            "games/system/interlude.json",
            "games/second/boss.json",
            "games/second/game-over.json",
            "games/system/game-over.json",
            "games/system/choose-mode.json",
        ];

        log::debug!("Declaring coroutine");
        let resources_loading: Coroutine = start_coroutine(async move {
            log::debug!("Starting coroutine");

            async fn preload_games(
                game_filenames: Vec<&'static str>,
                games_to_preload: Vec<&'static str>,
            ) -> WeeResult<(
                HashMap<&'static str, GameData>,
                HashMap<&'static str, Assets>,
            )> {
                let games: HashMap<&'static str, GameData> = {
                    let mut loaded_data = HashMap::new();
                    let mut waiting_data = Vec::new();
                    for filename in &game_filenames {
                        waiting_data.push(GameData::load(filename));
                    }

                    let mut data = join_all(waiting_data).await;

                    for filename in &game_filenames {
                        log::debug!("{}", filename);
                        loaded_data.insert(*filename, data.remove(0).unwrap());
                    }

                    loaded_data
                };

                let mut preloaded_assets = HashMap::new();
                let mut waiting_data = Vec::new();
                for filename in &games_to_preload {
                    waiting_data.push(LoadedGameData::load(filename));
                }

                let mut data = join_all(waiting_data).await;

                for filename in games_to_preload {
                    let loaded_data = data.remove(0).unwrap();
                    let assets = Assets {
                        images: loaded_data.images,
                        sounds: loaded_data.sounds,
                        music: loaded_data.music,
                        fonts: loaded_data.fonts,
                    };
                    preloaded_assets.insert(filename, assets);
                }

                log::debug!("Loaded games");

                Ok((games, preloaded_assets))
            }

            dispenser::store(preload_games(game_filenames, games_to_preload).await);
        });

        clear_background(WHITE);

        log::debug!("Started intro");

        assets.music.play(DEFAULT_PLAYBACK_RATE, VOLUME);

        while !resources_loading.is_done() {
            update_frame(&mut game, &assets, DEFAULT_PLAYBACK_RATE)?;

            draw_game(&game, &assets.images, &assets.fonts, &intro_font);

            next_frame().await;
        }

        assets.stop_sounds();

        let (games, preloaded_assets) = dispenser::take::<
            WeeResult<(
                HashMap<&'static str, GameData>,
                HashMap<&'static str, Assets>,
            )>,
        >()?;

        Ok(MainGame {
            state: Menu {},
            intro_font,
            games,
            preloaded_assets,
            high_scores: HashMap::new(),
            played_games: HashSet::new(),
        })
    }
}

struct Menu {}

impl MainGame<Menu> {
    async fn run_game_loop(self) -> WeeResult<MainGame<Menu>> {
        let main_game = self
            .pick_games()
            .await?
            .start()
            .await?
            .play_games()
            .await?
            .return_to_menu()
            .await?;
        Ok(main_game)
    }

    async fn pick_games(self) -> WeeResult<MainGame<Prelude>> {
        log::debug!("pick_games");
        let filename = "games/system/choose-mode.json";

        let mut game_data = self.games[filename].clone();
        let assets = &self.preloaded_assets[filename];

        {
            let text_replacements =
                vec![("{GamesCount}", format!("{}/41", self.played_games.len()))];
            for object in game_data.objects.iter_mut() {
                object.replace_text(&text_replacements);
            }
        }

        let mut game = Game::from_data(game_data);

        assets.music.play(DEFAULT_PLAYBACK_RATE, VOLUME);

        let directory;

        'choose_mode_running: loop {
            update_frame(&mut game, assets, DEFAULT_PLAYBACK_RATE)?;

            draw_game(&game, &assets.images, &assets.fonts, &self.intro_font);

            next_frame().await;

            for (key, object) in game.objects.iter() {
                if object.switch == SwitchState::SwitchedOn {
                    let pattern = "OpenFolder:";
                    if key.starts_with(pattern) {
                        directory = key[pattern.len()..].to_string();
                        break 'choose_mode_running;
                    }
                    if key == "Shuffle" {
                        directory = "games".to_string();
                        break 'choose_mode_running;
                    }
                }
            }
        }

        assets.stop_sounds();

        Ok(MainGame {
            state: Prelude { directory },
            intro_font: self.intro_font,
            games: self.games,
            preloaded_assets: self.preloaded_assets,
            high_scores: self.high_scores,
            played_games: self.played_games,
        })
    }
}

struct Prelude {
    directory: String,
}

impl MainGame<Prelude> {
    async fn start(self) -> WeeResult<MainGame<Interlude>> {
        log::debug!("prelude");

        let (game, assets) = preloaded_game(
            &self.games,
            &self.preloaded_assets,
            &self.state.directory,
            "prelude.json",
        );

        let mut game = Game::from_data(game);

        assets.music.play(DEFAULT_PLAYBACK_RATE, VOLUME);

        while game.frames.remaining() != FrameCount::Frames(0) && !game.end_early {
            update_frame(&mut game, assets, DEFAULT_PLAYBACK_RATE)?;

            draw_game(&game, &assets.images, &assets.fonts, &self.intro_font);

            next_frame().await;
        }

        assets.stop_sounds();

        let games_list = GamesList::from_directory(&self.games, self.state.directory);

        Ok(MainGame {
            state: Interlude {
                progress: Progress::new(),
                games_list,
            },
            intro_font: self.intro_font,
            games: self.games,
            preloaded_assets: self.preloaded_assets,
            high_scores: self.high_scores,
            played_games: self.played_games,
        })
    }
}

struct Interlude {
    progress: Progress,
    games_list: GamesList,
}

impl MainGame<Interlude> {
    async fn play_games(mut self) -> WeeResult<MainGame<GameOver>> {
        loop {
            let next_step = self.load_game().await?;
            self = match next_step {
                NextStep::Play(game) => game.play().await?,
                NextStep::Finished(game_over) => {
                    return Ok(game_over);
                }
            }
        }
    }

    async fn load_game(mut self) -> WeeResult<NextStep> {
        log::debug!("interlude");

        let progress = self.state.progress;
        let is_boss_game = progress.score > 0 && (progress.score % BOSS_GAME_INTERVAL == 0);

        let (mut game_data, assets) = preloaded_game(
            &self.games,
            &self.preloaded_assets,
            &self.state.games_list.directory,
            "interlude.json",
        );

        if self.state.progress.lives == 0 {
            {
                let text_replacements = vec![
                    ("{Score}", progress.score.to_string()),
                    ("{Lives}", progress.lives.to_string()),
                    ("{Game}", "game-over.json".to_string()),
                    ("{IntroText}", "Game Over".to_string()),
                ];
                for object in game_data.objects.iter_mut() {
                    let mut set_switch = |name, pred| {
                        if object.name == name {
                            object.switch = if pred { Switch::On } else { Switch::Off };
                        }
                    };
                    set_switch("1", progress.lives >= 1);
                    set_switch("2", progress.lives >= 2);
                    set_switch("3", progress.lives >= 3);
                    set_switch("4", progress.lives >= 4);

                    object.replace_text(&text_replacements);
                }
            }

            let mut game = Game::from_data(game_data);

            let playback_rate = self.state.progress.playback_rate;

            assets.music.play(playback_rate, VOLUME);

            while game.frames.remaining() != FrameCount::Frames(0) && !game.end_early {
                game.frames.steps_taken += 1;

                let frames_to_run = frames_to_run(game.frames, playback_rate);
                for _ in 0..frames_to_run {
                    update_frame(&mut game, assets, playback_rate)?;
                }

                draw_game(&game, &assets.images, &assets.fonts, &self.intro_font);

                next_frame().await;
            }

            assets.stop_sounds();

            for key in assets.sounds.keys() {
                macroquad::audio::stop_sound(assets.sounds[key]);
            }

            let next_step = NextStep::Finished(MainGame {
                state: GameOver {
                    progress: self.state.progress,
                    directory: self.state.games_list.directory,
                },
                intro_font: self.intro_font,
                games: self.games,
                preloaded_assets: self.preloaded_assets,
                high_scores: self.high_scores,
                played_games: self.played_games,
            });
            Ok(next_step)
        } else {
            let next_filename = if is_boss_game {
                self.state.games_list.choose_boss()
            } else {
                self.state.games_list.choose_game()
            };

            log::debug!("next filename: {}", next_filename);

            self.played_games.insert(next_filename);

            let new_game_data = self.games[next_filename].clone();

            {
                let text_replacements = vec![
                    ("{Score}", progress.score.to_string()),
                    ("{Lives}", progress.lives.to_string()),
                    (
                        "{Game}",
                        Path::new(&next_filename)
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    ),
                    (
                        "{IntroText}",
                        new_game_data
                            .intro_text
                            .as_deref()
                            .unwrap_or("")
                            .to_string(),
                    ),
                ];
                for object in game_data.objects.iter_mut() {
                    let mut set_switch = |name, pred| {
                        if object.name == name {
                            object.switch = if pred { Switch::On } else { Switch::Off };
                        }
                    };
                    if let Some(last_game) = progress.last_game {
                        set_switch("Won", last_game.has_won);
                        set_switch("Gained Life", last_game.was_life_gained);
                    }
                    set_switch("1", progress.lives >= 1);
                    set_switch("2", progress.lives >= 2);
                    set_switch("3", progress.lives >= 3);
                    set_switch("4", progress.lives >= 4);
                    set_switch("Boss", is_boss_game);

                    object.replace_text(&text_replacements);
                }
            }

            let mut game = Game::from_data(game_data);

            let resources_loading = start_coroutine(async move {
                let base_path = Path::new(next_filename).parent().unwrap();
                let resources = Assets::load(&new_game_data.asset_files, base_path).await;
                dispenser::store(resources);
            });

            let playback_rate = self.state.progress.playback_rate;

            assets.music.play(playback_rate, VOLUME);

            while (game.frames.remaining() != FrameCount::Frames(0) && !game.end_early)
                || !resources_loading.is_done()
            {
                game.frames.steps_taken += 1;

                let frames_to_run = frames_to_run(game.frames, playback_rate);
                for _ in 0..frames_to_run {
                    update_frame(&mut game, assets, playback_rate)?;
                }

                draw_game(&game, &assets.images, &assets.fonts, &self.intro_font);

                next_frame().await;
            }

            assets.stop_sounds();

            let assets = dispenser::take::<WeeResult<Assets>>()?;

            let next_step = NextStep::Play(MainGame {
                state: Play {
                    game_data: self.games[next_filename].clone(),
                    assets,
                    progress: self.state.progress,
                    games_list: self.state.games_list,
                    is_boss_game,
                },
                intro_font: self.intro_font,
                games: self.games,
                preloaded_assets: self.preloaded_assets,
                high_scores: self.high_scores,
                played_games: self.played_games,
            });
            Ok(next_step)
        }
    }
}

enum NextStep {
    Play(MainGame<Play>),
    Finished(MainGame<GameOver>),
}

struct Play {
    game_data: GameData,
    assets: Assets,
    progress: Progress,
    games_list: GamesList,
    is_boss_game: bool,
}

impl MainGame<Play> {
    async fn play(mut self) -> WeeResult<MainGame<Interlude>> {
        log::debug!("play");
        log::debug!("playback rate: {}", self.state.progress.playback_rate);

        let mut game = Game::from_data(self.state.game_data);
        game.difficulty = self.state.progress.difficulty;

        let playback_rate = if self.state.is_boss_game {
            self.state.progress.boss_playback_rate
        } else {
            self.state.progress.playback_rate
        };
        self.state.assets.music.play(playback_rate, VOLUME);

        while game.frames.remaining() != FrameCount::Frames(0) && !game.end_early {
            game.frames.steps_taken += 1;

            let frames_to_run = frames_to_run(game.frames, playback_rate);
            for _ in 0..frames_to_run {
                update_frame(&mut game, &self.state.assets, playback_rate)?;
            }

            draw_game(
                &game,
                &self.state.assets.images,
                &self.state.assets.fonts,
                &self.intro_font,
            );

            next_frame().await;
        }

        self.state.assets.stop_sounds();

        let has_won = match game.status.next_frame {
            WinStatus::Won | WinStatus::HasBeenWon => true,
            _ => false,
        };
        self.state.progress.update(has_won, self.state.is_boss_game);

        Ok(MainGame {
            state: Interlude {
                progress: self.state.progress,
                games_list: self.state.games_list,
            },
            intro_font: self.intro_font,
            games: self.games,
            preloaded_assets: self.preloaded_assets,
            high_scores: self.high_scores,
            played_games: self.played_games,
        })
    }
}

struct GameOver {
    progress: Progress,
    directory: String,
}

impl MainGame<GameOver> {
    async fn return_to_menu(mut self) -> WeeResult<MainGame<Menu>> {
        log::debug!("return to menu");

        let (mut game_data, assets) = preloaded_game(
            &self.games,
            &self.preloaded_assets,
            &self.state.directory,
            "game-over.json",
        );

        let mut high_scores = self
            .high_scores
            .entry(self.state.directory)
            .or_insert((0, 0, 0));
        let progress = self.state.progress;
        let score = progress.score;

        let high_score_position: Option<i32> = if score >= high_scores.0 {
            high_scores.2 = high_scores.1;
            high_scores.1 = high_scores.0;
            high_scores.0 = score;
            Some(1)
        } else if score >= high_scores.1 {
            high_scores.2 = high_scores.1;
            high_scores.1 = score;
            Some(2)
        } else if score >= high_scores.2 {
            high_scores.2 = score;
            Some(3)
        } else {
            None
        };

        let text_replacements = vec![
            ("{Score}", progress.score.to_string()),
            ("{Lives}", progress.lives.to_string()),
            ("{1st}", high_scores.0.to_string()),
            ("{2nd}", high_scores.1.to_string()),
            ("{3rd}", high_scores.2.to_string()),
        ];
        for object in game_data.objects.iter_mut() {
            object.replace_text(&text_replacements);

            let mut set_switch = |name, pred| {
                if object.name == name {
                    object.switch = if pred { Switch::On } else { Switch::Off };
                }
            };
            set_switch("1st", high_score_position == Some(1));
            set_switch("2nd", high_score_position == Some(2));
            set_switch("3rd", high_score_position == Some(3));
        }

        let mut game = Game::from_data(game_data);

        assets.music.play(1.0, VOLUME);

        while game.frames.remaining() != FrameCount::Frames(0) && !game.end_early {
            update_frame(&mut game, assets, DEFAULT_PLAYBACK_RATE)?;

            draw_game(&game, &assets.images, &assets.fonts, &self.intro_font);

            next_frame().await;
        }

        assets.stop_sounds();

        Ok(MainGame {
            state: Menu {},
            intro_font: self.intro_font,
            games: self.games,
            preloaded_assets: self.preloaded_assets,
            high_scores: self.high_scores,
            played_games: self.played_games,
        })
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Weegames Demo".to_string(),
        window_width: 1600,
        window_height: 900,
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    log::debug!("Start game");

    macroquad::rand::srand(macroquad::miniquad::date::now() as _);

    let camera = macroquad::camera::Camera2D::from_display_rect(macroquad::math::Rect::new(
        0.0,
        0.0,
        PROJECTION_WIDTH,
        PROJECTION_HEIGHT,
    ));
    macroquad::camera::set_camera(&camera);

    let main_game = MainGame::<LoadingScreen>::load().await;

    let mut main_game = match main_game {
        Ok(main_game) => main_game,
        Err(error) => {
            let error = format!("Failed to load game: {}", error);
            for _ in 0..300 {
                clear_background(BLACK);
                macroquad::text::draw_text(&error, 0.0, 64.0, 64.0, WHITE);
                next_frame().await;
            }
            panic!("Failed to load game");
        }
    };

    loop {
        let result = main_game.run_game_loop().await;
        main_game = match result {
            Ok(main_game) => main_game,
            Err(error) => {
                let error = format!("Failed to load game: {}", error);
                for _ in 0..300 {
                    clear_background(BLACK);
                    macroquad::text::draw_text(&error, 0.0, 64.0, 64.0, WHITE);
                    next_frame().await;
                }
                MainGame::<LoadingScreen>::load().await.unwrap()
            }
        }
    }
}
