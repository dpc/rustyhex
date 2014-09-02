// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

use cgmath;
use cgmath::FixedArray;
use cgmath::{Matrix, Matrix4, Point3, Vector3, Vector4};
use cgmath::Point as CgPoint;
use cgmath::{Transform, AffineMatrix3};
use cgmath::Vector;
use creature::Creature;
use creature::{Grunt, Scout, Heavy, Human};
use device;
use device::draw::CommandBuffer;
use gfx::GlCommandBuffer;
use gfx::GlDevice;
use game::Action;
use game::GameState;
use game::{Run, Move, Turn, Melee, Wait};
use gfx;
use gfx::{Device, DeviceHelper};
use hex2d::{Forward, Backward, Left, Right, Direction};
use hex2d::{North, Position, Point};
use input::keyboard as key;
use map::{Wall, Sand, GlassWall, Floor};
use piston;
use sdl2_game_window::GameWindowSDL2 as Window;
use std;
use std::collections::{RingBuf, Deque};
use std::num::{zero, one};
use time;

use piston::{
    GameIterator,
    GameIteratorSettings,
    GameWindowSettings,
    Render,
    Update,
    Input,
};

use piston::input::{
    InputEvent,
    Press,
    Release,
    Keyboard,
};

use std::mem::size_of;

#[vertex_format]
struct Vertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [f32, ..3],
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader. Its argument is the name of the type that will
// be generated to represent your the program. Search for `Batch` below, to
// see how it's used.
#[shader_param(Batch)]
struct Params {
    #[name = "u_Projection"]
    projection: [[f32, ..4], ..4],

    #[name = "u_View"]
    view: [[f32, ..4], ..4],

    #[name = "u_Model"]
    model: [[f32, ..4], ..4],

   #[name = "u_Color"]
    color: [f32, ..3],

}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core

    in vec3 a_Pos;

    smooth out vec4 v_Color;

    uniform mat4 u_Projection;
    uniform mat4 u_View;
    uniform mat4 u_Model;
    uniform vec3 u_Color;

    void main() {
        v_Color = vec4(u_Color, 1.0);
        gl_Position = u_Projection * u_View * u_Model * vec4(a_Pos, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core

    in vec4 v_Color;
    out vec4 o_Color;

    void main() {
        o_Color = v_Color;
    }
"
};

struct Renderer<C : device::draw::CommandBuffer, D: gfx::Device<C>> {
    graphics: gfx::Graphics<D, C>,
    tile_batch: Batch,
    projection: Matrix4<f32>,
    view: Matrix4<f32>,
    frame: gfx::Frame,
    cd: gfx::ClearData,
}

type Color = [f32, ..3];
static background_color: Color = [0.0f32, 0.0, 0.0];
static player_color : Color = [0.0f32, 0.0, 1.0];
static wall_color : Color = [0.3f32, 0.2, 0.0];
static glasswall_color : Color = [0.7f32, 0.7, 0.95];
static sand_color : Color = [1.0f32, 1.0, 0.8];
static floor_color : Color = [1.0f32, 0.9, 0.9];
static scout_color : Color = [0.0f32, 0.8, 0.0];
static grunt_color : Color = [0.0f32, 0.6, 0.0];
static heavy_color : Color = [0.0f32, 0.4, 0.0];
static tile_hight : f32 = 0.2f32;
static hack_player_knows_all : bool = false;
static hack_player_sees_everyone : bool = false;

fn grey_out(c : Color) -> Color {
    let [r, g, b ]  = c;
    [ r/2.0f32, g/2.0f32, b/2.0f32]
}
static billion : f32 = 1000000000f32;
static tau : f32 = std::f32::consts::PI_2;
static tile_outer_r : f32 = 1.0f32;
//static tile_inner_r : f32 = tile_outer_r * 3f32.sqrt() / 2f32;

fn tile_inner_r() -> f32 {
    tile_outer_r * 3f32.sqrt() / 2f32
}

#[allow(dead_code)]
fn edge_to_angle(i : uint) -> f32 {
    i as f32 * tau / 6.0f32
}

#[allow(dead_code)]
fn side_to_angle(i : uint) -> f32 {
    i as f32 * tau / 6.0f32 + tau / 12f32
}

pub fn point_to_pixel(p : Point) -> (f32, f32) {
    (
        p.x as f32 * tile_outer_r * 3f32 / 2f32,
        -((p.y * 2) as f32 + p.x as f32) * tile_inner_r()
    )
}

impl<C : CommandBuffer, D: gfx::Device<C>> Renderer<C, D> {
    fn new(mut device: D, frame: gfx::Frame) -> Renderer<C, D> {

        let (w, h) = (frame.width, frame.height);

        let vertex_data = Vec::from_fn(12, |i| {

            let angle = i as f32 * tau / 6.0f32;

            let px = tile_outer_r * angle.cos();
            let py = tile_outer_r * - angle.sin();
            Vertex { pos: [px, py, if i < 6 { tile_hight } else {0f32} ] }
        });

        let mesh = device.create_mesh(vertex_data);

        let index_data: Vec<u8> = vec!(
            5, 4, 3,
            5, 3, 2,
            5, 2, 1,
            5, 1, 0,
            6, 0, 1,
            7, 6, 1,
            7, 1, 2,
            8, 7, 2,
            8, 2, 3,
            9, 8, 3,
            9, 3, 4,
            10, 9, 4,
            10, 4, 5,
            11, 10,5,
            11, 5, 0,
            6, 11, 0,
            );

        let slice = {
            let buf = device.create_buffer_static(&index_data.as_slice());
            gfx::IndexSlice8(gfx::TriangleList, buf, 0, index_data.len() as u32)
        };

        let program = device.link_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone())
            .unwrap();
        let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true).multi_sample();

        let mut graphics = gfx::Graphics::new(device);
        let tile : Batch = graphics.make_batch(&program, &mesh, slice, &state).unwrap();

        let aspect = w as f32 / h as f32;
        let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 100.0);

        let [cr, cg, cb] = background_color;
        let clear_color = [cr, cg, cb, 1.0f32];

        Renderer {
            graphics: graphics,
            frame: frame,
            tile_batch : tile,
            projection: proj,
            view: proj,
            cd: gfx::ClearData {
                color: Some(clear_color),
                depth: Some(1.0),
                stencil: None,
            },
        }
    }

    fn render_params(&self, px : f32, py : f32, pz : f32, color : Color) -> Params {
        let mut model = Matrix4::identity();
        model[3] = Vector4::new(px, py, pz, 1.0f32);
        Params {
            projection: self.projection.into_fixed(),
            view: self.view.into_fixed(),
            color : color,
            model: model.into_fixed(),
        }
    }

    fn set_view(&mut self, view: &AffineMatrix3<f32>) {
        self.view = view.mat;
    }

    /// Clear
    fn clear(&mut self) {
        self.graphics.clear(self.cd, &self.frame);
    }

    fn end_frame(&mut self) {
        self.graphics.end_frame();
    }

    fn render_batch(&mut self, batch : &Batch, params : &Params) {
        self.graphics.draw(batch, params, &self.frame);
    }

    pub fn render_tile(&mut self, p : Point, c : Color, elevate : bool) {
        let (px, py) = point_to_pixel(p);
        let params = self.render_params(px, py, if elevate {tile_hight} else {0.0}, c);
        let batch = self.tile_batch;
        self.render_batch(&batch, &params);
    }

    pub fn render_creature(&mut self, p : Point, c : Color) {
        let (px, py) = point_to_pixel(p);
        let params = self.render_params(px, py, tile_hight, c);
        let batch = self.tile_batch;
        self.render_batch(&batch, &params);
    }


}


/// linearly interpolate between two values
///
/// s =
fn mix<F : FloatMath> (x : F, y : F, a : F) -> F {
    assert!(a >= zero());
    assert!(a <= one());

    y * a + x * (one::<F>() - a)
}

struct SmoothMovement<F, T> {
    destination: T,
    source: T,
    pub current: T,
    current_duration: F,
    duration: F,
}

impl<F : Float + FloatMath + cgmath::BaseNum, V : cgmath::Vector<F>, T : cgmath::Point<F, V>> SmoothMovement<F, T> {

    pub fn new(duration : F) -> SmoothMovement<F,T> {
        SmoothMovement {
            destination: cgmath::Point::origin(),
            source: cgmath::Point::origin(),
            current:  cgmath::Point::origin(),
            duration: duration,
            current_duration : duration,
        }
    }

    pub fn update(&mut self, dt : F) {
        if self.current_duration > zero() {
            self.current_duration = self.current_duration - dt;
            self.current_duration = self.current_duration.max(zero());

            let d = self.current_duration / self.duration;
            self.current = self.destination.add_v(&self.destination.sub_p(&self.source).mul_s(-d * d));
        }
    }

    pub fn set_destination(&mut self, dest : T) {
        let current = self.current.clone();
        self.source = current;
        self.destination = dest;
        self.current_duration = self.duration;
    }

    pub fn finish_immediately(&mut self) {
        self.current_duration = zero();
        self.current = self.destination.clone();
        self.source = self.destination.clone();
    }
}

pub struct PistonUI {
    renderer : Renderer<GlCommandBuffer, GlDevice>,
    render_controller : RenderController,
    input_controller: InputController,
    window : Window,
}

pub struct RenderController {
    player_pos: Position,
    camera_pos : SmoothMovement<f32, Point3<f32>>,
    camera_focus : SmoothMovement<f32, Point3<f32>>,
}

pub struct InputController {
    shift_pressed: bool,
    alt_pressed: bool,
    ctrl_pressed: bool,
    is_running: bool,
    action_queue: RingBuf<Action>,
}

impl InputController {
    pub fn new() -> InputController {
        InputController {
            shift_pressed: false,
            alt_pressed: false,
            ctrl_pressed: false,
            is_running: true,
            action_queue: RingBuf::new(),
        }
    }

    fn move_or_run(&self, dir : Direction) -> Action {
        if self.is_running {
            Run(dir)
        } else {
            Move(dir)
        }
    }

    fn push_move_or_run(&mut self, dir : Direction) {
        let a = self.move_or_run(dir);
        self.action_queue.push(a)
    }

    fn push_turn(&mut self, dir : Direction) {
        self.action_queue.push(Turn(dir))
    }

    fn push_melee(&mut self, dir : Direction) {
        self.action_queue.push(Melee(dir))
    }

    fn push_wait(&mut self) {
        self.action_queue.push(Wait)
    }

    pub fn push_input(&mut self, i : InputEvent) {
        match i {
            Press(Keyboard(k)) => {
                match (k, self.shift_pressed, self.ctrl_pressed) {
                    (key::LShift, _, _) => self.shift_pressed = true,
                    (key::RShift, _, _) => self.shift_pressed = true,
                    (key::LAlt, _, _)   => self.alt_pressed = true,
                    (key::RAlt, _, _)   => self.alt_pressed = true,
                    (key::LCtrl, _, _)  => self.ctrl_pressed = true,
                    (key::RCtrl, _, _)  => self.ctrl_pressed = true,
                    (key::R, _, _)      => self.is_running = !self.is_running,
                    (key::K, _, false)    => self.push_move_or_run(Forward),
                    (key::L, true, false) => self.push_move_or_run(Right),
                    (key::H, true, false) => self.push_move_or_run(Left),
                    (key::J, _, false)    => self.push_move_or_run(Backward),
                    (key::L, false, false) => self.push_turn(Right),
                    (key::H, false, false) => self.push_turn(Left),
                    (key::K, _, true)    => self.push_melee(Forward),
                    (key::L, _, true) => self.push_melee(Right),
                    (key::H, _, true) => self.push_melee(Left),
                    (key::Period, _, _) => self.push_wait(),
                    _ => { }
                }
            },
            Release(Keyboard(k)) => {
                match k {
                    key::LShift|key::RShift => {
                        self.shift_pressed = false
                    },
                    key::LAlt|key::RAlt => {
                        self.alt_pressed = false
                    },
                    key::LCtrl|key::RCtrl=> {
                        self.ctrl_pressed = false
                    },
                    _ => {}
                }
            },
            _ => {}
        }
    }

    pub fn pop_action(&mut self) -> Option<Action> {
        self.action_queue.pop_front()
    }
}

impl RenderController {
    fn new() -> RenderController {
        let cp = SmoothMovement::new(3f32);
        let cf = SmoothMovement::new(1f32);
        RenderController {
            player_pos: Position::new(Point::new(0,0), North),
            camera_pos: cp,
            camera_focus: cf,
        }
    }

    pub fn render_map(
        &self,
        renderer : &mut Renderer<GlCommandBuffer, GlDevice>, game : &GameState) {
        let &GameState {
            ref player,
            ..
        } = game;


        let player = player.as_ref().and_then(|pl| pl.upgrade());
        let player = player.as_ref().and_then(|pl| pl.try_borrow());

        game.map.for_each_point(|ap| {

            if player.as_ref().map_or(true, |pl| pl.knows(ap) || hack_player_knows_all) {
                let tiletype = game.map.at(ap).tiletype;
                let (color, elevate) = match tiletype {
                    Wall => (wall_color, true),
                    GlassWall => (glasswall_color, true),
                    Floor => (floor_color, false),
                    Sand => (sand_color, false),
                };

                let color = if player.as_ref().map_or(
                    false, |pl| !pl.sees(ap)
                    ) {
                    grey_out(color)
                } else {
                    color
                };

                renderer.render_tile(ap, color, elevate);
            };
        });

        game.map.for_each_point(|ap| {
            if game.map.at(ap).creature.is_none()
                || !player.as_ref().map_or(
                    true, |pl| pl.sees(ap) || hack_player_sees_everyone
                    ) {
                return;
            }

            let creature = game.map.at(ap).creature.as_ref().unwrap();
            let creature = creature.borrow();

            let color = self.creature_color(&*creature);
            renderer.render_creature(ap, color);
        });
    }

    fn creature_color(&self, cr : &Creature) -> Color {
        let now_ns = time::precise_time_ns();
        let duration_s = 0.8f32;

        let base_color = if cr.is_player() {
            player_color
        } else {
            match cr.race() {
                Scout => scout_color,
                Grunt => grunt_color,
                Heavy => heavy_color,
                Human => fail!(),
            }
        };

        let last_time_s = (now_ns - cr.was_attacked_ns()) as f32 / billion;
        if last_time_s < duration_s {
            let f = last_time_s / duration_s;
            [
                mix(1f32, base_color[0], f),
                mix(0f32, base_color[1], f),
                mix(0f32, base_color[2], f),
            ]
        } else {
            base_color
        }
    }

    fn move_camera_to_destination(&mut self) {
        self.camera_pos.finish_immediately();
        self.camera_focus.finish_immediately();
    }

    fn set_player_pos(&mut self, pl: &Creature) {
        let pos = *pl.pos();
        if self.player_pos == pos {
            return;
        }
        self.player_pos = pos;

        let front = pos.p + pos.dir;

        let (fx, fy) = point_to_pixel(front);
        let (x, y) = point_to_pixel(pos.p);
        let (dx, dy) = (fx - x,  fy - y);
        let how_much_behind = 6f32;
        let how_much_front = 3f32;
        let (dbx, dby) = (dx * how_much_behind, dy * how_much_behind);
        let (dfx, dfy) = (dx * how_much_front, dy * how_much_front);
        self.camera_pos.set_destination(Point3::new(x - dbx, y - dby, 10.0));
        self.camera_focus.set_destination(Point3::new(x + dfx, y + dfy, 0.0));
    }

    fn update_movement(&mut self, dt : f32) {
        self.camera_pos.update(dt);
        self.camera_focus.update(dt);
    }

    fn update_camera(&mut self, renderer : &mut Renderer<GlCommandBuffer, GlDevice>) {

        let view : AffineMatrix3<f32> = Transform::look_at(
            &self.camera_pos.current,
            &self.camera_focus.current,
            &Vector3::unit_z(),
            );
        renderer.set_view(&view);
    }
}

impl PistonUI {
    pub fn new() -> PistonUI {
        let window = Window::new(
            piston::shader_version::opengl::OpenGL_3_2,
            GameWindowSettings {
                title: "Rustyhex".to_string(),
                size: [800, 600],
                fullscreen: false,
                exit_on_esc: true
            }
            );

        let (device, frame) = window.gfx();

        let renderer = Renderer::new(device, frame);

        PistonUI {
            render_controller: RenderController::new(),
            input_controller: InputController::new(),
            window: window,
            renderer: renderer,
        }
    }

    pub fn run (&mut self, game : &mut GameState) {
        let game_iter_settings = GameIteratorSettings {
            updates_per_second: 60,
            max_frames_per_second: 60,

        };

        game.update_player_los();

        let &PistonUI {
            ref mut renderer,
            ref mut render_controller,
            ref mut input_controller,
            ref mut window,
        } = self;

        {
            let pl = game.player.as_ref().and_then(|pl| pl.upgrade());
            if pl.is_some() {
                let pl = pl.unwrap();
                render_controller.set_player_pos(&*pl.borrow());
                render_controller.move_camera_to_destination();
                render_controller.update_camera(renderer);
            }
        }

        let mut render_time = time::precise_time_ns();

        let mut events = GameIterator::new(window, &game_iter_settings);
        for e in events {

            let pl = game.player.as_ref().and_then(|pl| pl.upgrade());
            let player_needs_input = pl.as_ref().map(|pl| pl.borrow().needs_action()).unwrap_or(false);

            match e {
                Render(_) => {
                    let t = time::precise_time_ns();
                    let dt = t - render_time;
                    render_time = t;
                    render_controller.update_movement(dt as f32 / billion as f32);
                    render_controller.update_camera(renderer);
                    renderer.clear();
                    render_controller.render_map(renderer, game);
                    renderer.end_frame();
                },
                Update(_) => {
                    if player_needs_input {
                        match input_controller.pop_action() {
                            Some(action) => {
                                pl.as_ref().map(|pl| pl.borrow_mut().action_set(action));
                            },
                            _ => {
                                continue;
                            }
                        }
                    }
                    game.tick();
                    if pl.is_some() {
                        let pl = pl.unwrap();
                        render_controller.set_player_pos(&*pl.borrow());
                    }
                },
                Input(i) => {
                    input_controller.push_input(i);
                }
            }
        }
    }

}
