use macroquad::prelude::*;
use macroquad::texture;
use quad_snd::mixer::Sound;

use futures::future::join_all;
use std::{collections::HashMap, default::Default, path::Path, str};

mod wee;

use wee::*;

const PROJECTION_WIDTH: f32 = 1600.0;
const PROJECTION_HEIGHT: f32 = 900.0;

const DEFAULT_DIFFICULTY: u32 = 1;
const MAX_LIVES: i32 = 4;
const INITIAL_PLAYBACK_RATE: f32 = 1.0;
const PLAYBACK_RATE_INCREASE: f32 = 0.1;
const PLAYBACK_RATE_MAX: f32 = 2.0;
const UP_TO_DIFFICULTY_TWO: i32 = 20;
const UP_TO_DIFFICULTY_THREE: i32 = 40;
const BOSS_GAME_INTERVAL: i32 = 15;
const INCREASE_SPEED_AFTER_GAMES: i32 = 5;

enum GameMode {
    Prelude,
    Interlude,
    Play,
    Boss,
    GameOver,
    PlayAgain,
}

async fn load_images(image_files: &HashMap<String, String>) -> Images {
    macroquad::logging::debug!("Start loading images");
    let mut images = Images::new();
    let mut paths = Vec::new();
    let mut loading_images = Vec::new();

    let base_path = Path::new("demo-games/images");

    for path in image_files.values() {
        let path = base_path.join(path);

        paths.push(path.to_str().unwrap().to_string());
    }

    for path in &paths {
        loading_images.push(texture::load_texture(path));
    }

    let textures: Vec<_> = join_all(loading_images).await;

    for (key, texture) in image_files.keys().zip(textures) {
        unsafe {
            let context = macroquad::window::get_internal_gl().quad_context;
            texture.set_filter(context, macroquad::texture::FilterMode::Nearest);
        }
        images.insert(key.to_string(), texture);
    }

    images
}

async fn load_sounds(sound_files: &HashMap<String, String>) -> Sounds {
    let base_path = Path::new("demo-games/audio");
    let mut sounds = Sounds::new();

    for (key, filename) in sound_files {
        let path = base_path.join(&filename);

        let data = macroquad::file::load_file(&path.to_str().unwrap())
            .await
            .unwrap();

        let sound = quad_snd::decoder::read_ogg(&data).unwrap();

        sounds.insert(key.to_string(), sound);
    }
    sounds
}

#[derive(Clone)]
struct Music {
    data: Sound,
    looped: bool,
}

async fn load_music(music_file: &Option<SerialiseMusic>) -> Option<Music> {
    let base_path = Path::new("demo-games/audio");

    if let Some(music_info) = music_file {
        let path = base_path.join(&music_info.filename);

        let data = macroquad::file::load_file(&path.to_str().unwrap())
            .await
            .unwrap();

        let mut sound = quad_snd::decoder::read_ogg(&data).unwrap();
        if music_info.looped {
            sound.playback_style = quad_snd::mixer::PlaybackStyle::Looped;
        }

        Some(Music {
            data: sound,
            looped: music_info.looped,
        })
    } else {
        None
    }
}

async fn load_fonts(font_files: &HashMap<String, FontLoadInfo>) -> Fonts {
    let base_path = Path::new("demo-games/fonts");
    let mut fonts = Fonts::new();

    for (key, font_info) in font_files {
        let path = base_path.join(&font_info.filename);

        let font = macroquad::text::load_ttf_font(path.to_str().unwrap()).await;
        fonts.insert(key.to_string(), (font, font_info.size as u16));
    }
    fonts
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
    async fn load(filename: &str) -> LoadedGameData {
        let game_data = GameData::load(filename).await.unwrap();
        LoadedGameData {
            images: load_images(&game_data.asset_files.images).await,
            music: load_music(&game_data.asset_files.music).await,
            sounds: load_sounds(&game_data.asset_files.audio).await,
            fonts: load_fonts(&game_data.asset_files.fonts).await,
            data: game_data,
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
    next_boss_game: i32,
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
            next_boss_game: BOSS_GAME_INTERVAL,
        }
    }

    fn get_playback_rate(&self, game_mode: &GameMode) -> f32 {
        match game_mode {
            GameMode::Play => self.playback_rate,
            GameMode::Boss => self.boss_playback_rate,
            _ => 1.0,
        }
    }

    fn update(&mut self, has_won: bool, playback_increase: f32, playback_max: f32) {
        if has_won {
            self.score += 1;
            if self.score % INCREASE_SPEED_AFTER_GAMES == 0 {
                self.playback_rate += playback_increase;
            }
            if self.score >= UP_TO_DIFFICULTY_THREE {
                self.difficulty = 3;
            } else if self.score >= UP_TO_DIFFICULTY_TWO {
                self.difficulty = 2;
            }
            self.playback_rate = self.playback_rate.min(playback_max);
        } else {
            self.lives -= 1;
        }
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

// Adapted from draw_texture_ex in macroquad
fn draw_texture_ex2(
    texture: Texture2D,
    x: f32,
    y: f32,
    color: Color,
    params: DrawTextureParams,
    flip: Flip,
) {
    unsafe {
        let gl = macroquad::window::get_internal_gl().quad_gl;

        let macroquad::math::Rect {
            x: sx,
            y: sy,
            w: sw,
            h: sh,
        } = params.source.unwrap_or(macroquad::math::Rect {
            x: 0.,
            y: 0.,
            w: texture.width(),
            h: texture.height(),
        });

        let (w, h) = match params.dest_size {
            Some(dst) => (dst.x, dst.y),
            _ => (sw, sh),
        };

        let pivot = params.pivot.unwrap_or(vec2(x + w / 2., y + h / 2.));
        let m = pivot;
        let p = [
            vec2(x, y) - pivot,
            vec2(x + w, y) - pivot,
            vec2(x + w, y + h) - pivot,
            vec2(x, y + h) - pivot,
        ];
        let r = if flip.horizontal == flip.vertical {
            params.rotation
        } else {
            -params.rotation
        };
        let mut p = [
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
        use std::ptr::swap;
        if flip.horizontal {
            swap(&mut p[0].x, &mut p[2].x);
            swap(&mut p[1].x, &mut p[3].x);
        }
        if flip.vertical {
            swap(&mut p[0].y, &mut p[2].y);
            swap(&mut p[1].y, &mut p[3].y);
        }
        #[rustfmt::skip]
        let vertices = [
            Vertex::new(p[0].x, p[0].y, 0.,  sx      /texture.width(),  sy      /texture.height(), color),
            Vertex::new(p[1].x, p[1].y, 0., (sx + sw)/texture.width(),  sy      /texture.height(), color),
            Vertex::new(p[2].x, p[2].y, 0., (sx + sw)/texture.width(), (sy + sh)/texture.height(), color),
            Vertex::new(p[3].x, p[3].y, 0.,  sx      /texture.width(), (sy + sh)/texture.height(), color),
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        gl.texture(Some(texture));
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
                };
                draw_texture_ex2(
                    images[name],
                    part.area.min.x,
                    part.area.min.y,
                    macroquad::color::WHITE,
                    params,
                    Flip::default(),
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
                        };
                        draw_texture_ex2(
                            images[name],
                            object.position.x - object.size.width / 2.0,
                            object.position.y - object.size.height / 2.0,
                            macroquad::color::WHITE,
                            params,
                            object.flip,
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
                            object.position.y - size.1 / 1.25,
                        ),
                        JustifyText::Centre => wee::Vec2::new(
                            object.position.x - size.0 / 2.0,
                            object.position.y - size.1 / 2.0,
                        ),
                    };
                    let params = macroquad::text::TextParams {
                        font: fonts[&game.drawn_text[key].font].0,
                        font_size: fonts[&game.drawn_text[key].font].1,
                        font_scale: 1.0,
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
            color: colour,
        };
        macroquad::text::draw_text_ex(
            &game.intro_text,
            PROJECTION_WIDTH / 2.0 - size.0 / 2.0,
            PROJECTION_HEIGHT / 2.0 - size.1 / 2.0,
            params,
        );

        let colour = WHITE;
        let size = macroquad::text::measure_text(&game.intro_text, Some(*intro_font), 174, 1.0);
        let params = macroquad::text::TextParams {
            font: *intro_font,
            font_size: 174,
            font_scale: 1.0,
            color: colour,
        };
        macroquad::text::draw_text_ex(
            &game.intro_text,
            PROJECTION_WIDTH / 2.0 - size.0 / 2.0,
            PROJECTION_HEIGHT / 2.0 - size.1 / 2.0,
            params,
        );
    }
}

#[macroquad::main("Weegames Demo")]
async fn main() {
    /*for entry in std::fs::read_dir("demo-games/audio").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        println!("{:?}", path);
    }*/

    macroquad::logging::debug!("Start game");

    macroquad::rand::srand(macroquad::miniquad::date::now() as _);

    let game_filenames = vec![
        "demo-games/bike.json",
        "demo-games/break.json",
        "demo-games/dawn.json",
        "demo-games/disc.json",
        "demo-games/explosion.json",
        "demo-games/gravity.json",
        "demo-games/jump.json",
        "demo-games/knock.json",
        "demo-games/mine.json",
        "demo-games/racer.json",
        "demo-games/slide.json",
        "demo-games/stealth.json",
        "demo-games/swim.json",
        "demo-games/tanks.json",
        "demo-games/time.json",
    ];

    let mut games_to_load = game_filenames.clone();
    games_to_load.append(&mut vec![
        "demo-games/prelude.json",
        "demo-games/interlude.json",
        "demo-games/boss.json",
        "demo-games/game-over.json",
        "demo-games/play-again.json",
    ]);

    let mut loaded_data = HashMap::new();
    let mut waiting_data = Vec::new();
    for filename in &games_to_load {
        waiting_data.push(LoadedGameData::load(filename));
    }

    let mut data = join_all(waiting_data).await;

    for filename in &games_to_load {
        loaded_data.insert(filename.to_string(), data.remove(0));
    }

    macroquad::logging::debug!("Loaded games");

    let mut filename = "demo-games/prelude.json";
    let mut game_data = loaded_data[filename].data.clone();

    let mut game = Game::from_data(game_data);

    macroquad::logging::debug!("Game created");

    let mut images = loaded_data[filename].images.clone();

    macroquad::logging::debug!("Images added");

    let mut fonts = loaded_data[filename].fonts.clone();

    macroquad::logging::debug!("Fonts added");

    let mut sounds = loaded_data[filename].sounds.clone();
    let mut music = loaded_data[filename].music.clone();
    // music.start();

    macroquad::logging::debug!("Audio added");

    let camera = macroquad::camera::Camera2D::from_display_rect(macroquad::math::Rect::new(
        0.0,
        0.0,
        PROJECTION_WIDTH,
        PROJECTION_HEIGHT,
    ));
    macroquad::camera::set_camera(camera);

    let mut game_mode = GameMode::Prelude;

    let mut progress = Progress::new();

    let mut high_scores: (i32, i32, i32) = (0, 0, 0);

    let mut next_filename = "";
    let mut next_game_data;

    let mut next_games = Vec::new();

    fn choose_next_game(
        game_filenames: &[&'static str],
        next_games: &mut Vec<&'static str>,
    ) -> &'static str {
        if next_games.is_empty() {
            let mut potential_games = game_filenames.to_vec();
            for _ in 0..5 {
                let game = potential_games.remove(rand::gen_range(0, potential_games.len()));
                next_games.push(game);
            }
        }

        next_games.remove(0)
    }

    let intro_font = macroquad::text::load_ttf_font("fonts/Roboto-Medium.ttf").await;

    let mut mixer = quad_snd::mixer::SoundMixer::new();

    let mut music_id = None;
    let mut sound_ids = Vec::new();

    loop {
        mixer.frame();

        if FrameCount::Frames(0) == game.frames.remaining() || game.end_early {
            if let Some(sound_id) = music_id.take() {
                mixer.stop(sound_id);
            }
            for sound_id in &sound_ids {
                mixer.stop(*sound_id);
            }
            sound_ids = Vec::new();
            match game_mode {
                GameMode::Prelude => {
                    //filename = game_filenames.choose().unwrap();
                    filename = choose_next_game(&game_filenames, &mut next_games);
                    game_mode = GameMode::Play;
                    progress = Progress::new();
                }
                GameMode::Interlude => {
                    if progress.next_boss_game == progress.score {
                        filename = next_filename;
                        game_mode = GameMode::Boss;
                    } else {
                        filename = next_filename;
                        game_mode = GameMode::Play;
                        macroquad::logging::debug!("Playback rate: {}", progress.playback_rate);
                    }
                }
                GameMode::Play => {
                    let has_won = match game.status.next_frame {
                        WinStatus::Won | WinStatus::HasBeenWon => true,
                        _ => false,
                    };
                    if progress.next_boss_game == progress.score {
                        progress.next_boss_game += BOSS_GAME_INTERVAL;
                        progress.boss_playback_rate += PLAYBACK_RATE_INCREASE;
                    }
                    progress.update(has_won, PLAYBACK_RATE_INCREASE, PLAYBACK_RATE_MAX);
                    progress.last_game = Some(LastGame {
                        has_won,
                        was_life_gained: false,
                    });
                    if progress.lives <= 0 {
                        filename = "demo-games/game-over.json";
                        game_mode = GameMode::GameOver;
                    } else {
                        filename = "demo-games/interlude.json";
                        game_mode = GameMode::Interlude;
                    }
                }
                GameMode::Boss => {
                    let has_won = match game.status.next_frame {
                        WinStatus::Won | WinStatus::HasBeenWon => true,
                        _ => false,
                    };
                    progress.next_boss_game += BOSS_GAME_INTERVAL;
                    progress.update(has_won, PLAYBACK_RATE_INCREASE, PLAYBACK_RATE_MAX);
                    let was_life_gained = has_won && progress.lives < MAX_LIVES;
                    if was_life_gained {
                        progress.lives += 1;
                    }
                    progress.last_game = Some(LastGame {
                        has_won,
                        was_life_gained,
                    });
                    if progress.lives <= 0 {
                        filename = "demo-games/game-over.json";
                        game_mode = GameMode::GameOver;
                    } else {
                        filename = "demo-games/interlude.json";
                        game_mode = GameMode::Interlude;
                    }
                }
                GameMode::GameOver => {
                    filename = "demo-games/play-again.json";
                    game_mode = GameMode::PlayAgain;
                }
                GameMode::PlayAgain => {
                    filename = "demo-games/prelude.json";
                    game_mode = GameMode::Prelude;
                }
            }
            game_data = loaded_data[filename].data.clone();

            macroquad::logging::debug!("Game loaded");

            match game_mode {
                GameMode::Interlude => {
                    if progress.next_boss_game == progress.score {
                        next_filename = "demo-games/boss.json";
                        next_game_data = loaded_data[next_filename].data.clone();
                    } else {
                        //next_filename = game_filenames.choose().unwrap();
                        next_filename = choose_next_game(&game_filenames, &mut next_games);
                        next_game_data = loaded_data[next_filename].data.clone();
                    };

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
                            next_game_data
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
                        //set_switch("Boss", games_list.next_boss_game == progress.score);

                        object.replace_text(&text_replacements);
                    }
                }
                GameMode::GameOver => {
                    let high_score_position = if progress.score >= high_scores.0 {
                        high_scores.2 = high_scores.1;
                        high_scores.1 = high_scores.0;
                        high_scores.0 = progress.score;
                        Some(1)
                    } else if progress.score >= high_scores.1 {
                        high_scores.2 = high_scores.1;
                        high_scores.1 = progress.score;
                        Some(2)
                    } else if progress.score >= high_scores.2 {
                        high_scores.2 = progress.score;
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
                }
                _ => {}
            }

            game = Game::from_data(game_data);
            game.difficulty = progress.difficulty;

            macroquad::logging::debug!("Game created");

            images = loaded_data[filename].images.clone();
            fonts = loaded_data[filename].fonts.clone();
            sounds = loaded_data[filename].sounds.clone();
            music = loaded_data[filename].music.clone();
            if let Some(music) = &music {
                music_id = Some(mixer.play_ext(
                    music.data.clone(),
                    quad_snd::mixer::Volume(1.0),
                    quad_snd::mixer::Speed(progress.get_playback_rate(&game_mode)),
                ));
            }
        }

        clear_background(WHITE);

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

        game.frames.steps_taken += 1;

        let frames_to_run = frames_to_run(game.frames, progress.get_playback_rate(&game_mode));

        /*let frames_to_run = match game_mode {
            GameMode::Play | GameMode::Boss => frames_to_run(game.frames, progress.get_playback_rate()),
            _ => 1,
        };*/

        for _ in 0..frames_to_run {
            use macroquad::input::MouseButton;
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

            let played_sounds = game.update(&mouse);

            for played_sound in played_sounds {
                let sound_id = mixer.play_ext(
                    sounds[&played_sound].clone(),
                    quad_snd::mixer::Volume(1.0),
                    quad_snd::mixer::Speed(progress.get_playback_rate(&game_mode)),
                );
                sound_ids.push(sound_id);
            }

            if game.has_music_finished {
                if let Some(sound_id) = music_id {
                    mixer.stop(sound_id);
                }
            }

            game.status.current = game.status.next_frame;
            game.status.next_frame = match game.status.next_frame {
                WinStatus::HasBeenWon => WinStatus::Won,
                WinStatus::HasBeenLost => WinStatus::Lost,
                _ => game.status.next_frame,
            };
            game.frames.ran += 1;
        }

        draw_game(&game, &images, &fonts, &intro_font);

        next_frame().await
    }
}
