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
use std;
use glfw_game_window::WindowGLFW as Window;
use std::collections::{RingBuf, Deque};
use std::num::{zero, one};
use time;
use glfw;
use obj;
use genmesh;
use genmesh::Indexer;

use piston::{
    EventIterator,
    EventSettings,
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

    #[as_float]
    #[name = "a_Normal"]
    normal: [f32, ..3],
}

impl std::cmp::PartialEq for Vertex {
    fn eq(&self, other: &Vertex) -> bool {
        self.pos.as_slice() == other.pos.as_slice() &&
        self.normal.as_slice() == other.normal.as_slice()
    }
}

impl std::clone::Clone for Vertex {
    fn clone(&self) -> Vertex {
        Vertex {
            pos: self.pos,
            normal: self.normal
        }
    }
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
    color: [f32, ..4],

   #[name = "u_LightDirection"]
    light: [f32, ..3],
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core

    in vec3 a_Pos;
    in vec3 a_Normal;

    smooth out vec4 v_Color;

    uniform mat4 u_Projection;
    uniform mat4 u_View;
    uniform mat4 u_Model;
    uniform vec4 u_Color;
    uniform vec3 u_LightDirection;

    void main() {
        vec3 normal = normalize(vec3(u_Model * vec4(a_Normal, 0.0)));
        float dot = max(dot(normal, u_LightDirection), 0.0);
        v_Color = u_Color * (dot + 1) / 2;
        gl_Position = u_Projection * u_View * u_Model * vec4(a_Pos, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core

    smooth in vec4 v_Color;
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

type Color = [f32, ..4];
static background_color: Color = [0.0f32, 0.0, 0.0, 1.0];
static player_color : Color = [0.0f32, 0.0, 1.0, 1.0];
static wall_color : Color = [0.3f32, 0.2, 0.0, 1.0];
static glasswall_color : Color = [0.7f32, 0.7, 0.95, 1.0];
static sand_color : Color = [1.0f32, 1.0, 0.8, 1.0];
static floor_color : Color = [1.0f32, 0.9, 0.9, 1.0];
static scout_color : Color = [0.0f32, 0.8, 0.0, 1.0];
static grunt_color : Color = [0.0f32, 0.6, 0.0, 1.0];
static heavy_color : Color = [0.0f32, 0.4, 0.0, 1.0];
static tile_hight : f32 = 0.2f32;
static hack_player_knows_all : bool = false;
static hack_player_sees_everyone : bool = false;

fn grey_out(c : Color) -> Color {
    let [r, g, b, a]  = c;
    [ r/2.0f32, g/2.0f32, b/2.0f32, a]
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

type IndexVector = Vec<u8>;
type VertexVector = Vec<Vertex>;

pub fn load_hex() -> (IndexVector, VertexVector) {
    let obj = obj::load(&Path::new("assets/hex.obj")).unwrap();

    let mut index_data : Vec<u8> = vec!();
    let mut vertex_data : Vec<Vertex> = vec!();

    {
        let mut indexer = genmesh::LruIndexer::new(16, |_, v| {
            vertex_data.push(v);
        });

        for o in obj.object_iter() {
            for g in o.group_iter() {
                for i in g.indices().iter() {
                    match i {
                        &genmesh::PolyTri(poly) => {

                            for i in vec!(poly.x, poly.y, poly.z).iter() {
                                match i {
                                    &(v, _, Some(n)) => {
                                        let normal = obj.normal()[n];
                                        let vertex = obj.position()[v];
                                        let index = indexer.index(
                                            Vertex {
                                                pos: vertex,
                                                normal: normal,
                                            }
                                            );
                                        index_data.push(index as u8);
                                    },
                                    _ => { fail!() }
                                }
                            }


                        },
                        _ => { fail!() },
                    }
                }
            }
        }
    }
    (index_data, vertex_data)
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

        let (index_data, vertex_data) = load_hex();

        let mesh = device.create_mesh(vertex_data.as_slice());

        let slice = {
            let buf = device.create_buffer_static(index_data.as_slice());
            gfx::IndexSlice8(gfx::TriangleList, buf, 0, index_data.len() as u32)
        };

        let program = device.link_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone())
            .unwrap();
        let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true).multi_sample();

        let mut graphics = gfx::Graphics::new(device);
        let tile : Batch = graphics.make_batch(&program, &mesh, slice, &state).unwrap();

        let aspect = w as f32 / h as f32;
        let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 100.0);

        Renderer {
            graphics: graphics,
            frame: frame,
            tile_batch : tile,
            projection: proj,
            view: proj,
            cd: gfx::ClearData {
                color: background_color,
                depth: 1.0,
                stencil: 0,
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
            light: Vector3::unit_z().into_fixed(),
        }
    }

    fn set_view(&mut self, view: &AffineMatrix3<f32>) {
        self.view = view.mat;
    }

    /// Clear
    fn clear(&mut self) {
        self.graphics.clear(self.cd, gfx::Color | gfx::Depth, &self.frame);
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

struct SmoothMovement<T> {
    destination: T,
    source: T,
    pub current: T,
    current_duration: f32,
    duration: f32,
}

impl<V : cgmath::Vector<f32>, T : cgmath::Point<f32, V>> SmoothMovement<T> {

    pub fn new(duration : f32) -> SmoothMovement<T> {
        SmoothMovement {
            destination: cgmath::Point::origin(),
            source: cgmath::Point::origin(),
            current:  cgmath::Point::origin(),
            duration: duration,
            current_duration : duration,
        }
    }

    pub fn update(&mut self, dt : f32) {
        if self.current_duration > zero() {
            self.current_duration = self.current_duration - dt;
            self.current_duration = self.current_duration.max(zero());

            let d = self.current_duration / self.duration;
            // cos((x -1)* 3.146) / 2 + 0.5
            let d = ((d - 1f32)* Float::pi()).cos() / 2f32 + 0.5f32;
            self.current = self.destination.add_v(&self.destination.sub_p(&self.source).mul_s(-d));
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
}

pub struct RenderController {
    player_pos: Position,
    camera_pos : SmoothMovement<Point3<f32>>,
    camera_focus : SmoothMovement<Point3<f32>>,
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
        let cp = SmoothMovement::new(2.5f32);
        let cf = SmoothMovement::new(1.0f32);
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


        let player = player.as_ref().and_then(|pl| pl.try_borrow());

        game.map.for_each_point(|ap| {

            if player.as_ref().map_or(true, |pl| pl.knows(ap) || !pl.is_alive() || hack_player_knows_all) {
                let tiletype = game.map.at(ap).tiletype;
                let (color, elevate) = match tiletype {
                    Wall => (wall_color, true),
                    GlassWall => (glasswall_color, true),
                    Floor => (floor_color, false),
                    Sand => (sand_color, false),
                };

                let color = if player.as_ref().map_or(
                    false, |pl| !pl.sees(ap) && pl.is_alive()
                    ) {
                    grey_out(color)
                } else {
                    color
                };

                renderer.render_tile(ap, color, elevate);
            };
        });

        for creature in game.creatures_iter() {
            let creature = creature.borrow();

            let ap = creature.pos().p;


            if !player.as_ref().map_or(
                    true, |pl| pl.sees(ap) || !pl.is_alive() || hack_player_sees_everyone
                    ) {
                continue;
            }

            match self.creature_color(&*creature) {
                Some(color) => renderer.render_creature(ap, color),
                None => {}
            }
        };
    }

    fn creature_color(&self, cr : &Creature) -> Option<Color> {
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
        let color = base_color;

        let since_s = (now_ns - cr.was_attacked_ns()) as f32 / billion;
        let color = if since_s < duration_s {
            let f = since_s / duration_s;
            [
                mix(1f32, color[0], f),
                mix(0f32, color[1], f),
                mix(0f32, color[2], f),
                color[3],
            ]
        } else {
            color
        };

        let color = if !cr.is_alive() {
            let since_s = (now_ns - cr.death_ns()) as f32 / billion;
            let f = since_s / duration_s;
            if f < 1.0 {
                Some([
                    mix(color[0], floor_color[0], f),
                    mix(color[1], floor_color[1], f),
                    mix(color[2], floor_color[2], f),
                    color[3],
                ])
            } else {
                None
            }
        } else {
            Some(color)
        };

        color
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
    pub fn new() -> (PistonUI, Window) {

        // TODO: Consider simplifying after
        // https://github.com/PistonDevelopers/piston/issues/624
        // is implemented
        let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

        let width = 800;
        let height = 600;

        glfw.window_hint(glfw::ContextVersion(3, 2));
        glfw.window_hint(glfw::OpenglForwardCompat(true));
        glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));
        glfw.window_hint(glfw::Samples(4));

        let (window, events) = glfw
            .create_window(width, height, "Rustyhex", glfw::Windowed)
            .expect("Failed to create GLFW window.");

        let frame = gfx::Frame::new(width as u16, height as u16);
        let device = gfx::GlDevice::new(|s| window.get_proc_address(s));

        let window = Window::from_pieces(
            window, glfw, events, true
            );


        let renderer = Renderer::new(device, frame);

        (PistonUI {
            render_controller: RenderController::new(),
            input_controller: InputController::new(),
            renderer: renderer,
        }, window)
    }

    fn game_update(&mut self, game : &mut GameState) {
        let player_needs_input = game.player.as_ref().map(|pl| pl.borrow().needs_action()).unwrap_or(false);
        if player_needs_input {
            match self.input_controller.pop_action() {
                Some(action) => {
                    game.player.as_ref().map(|pl| pl.borrow_mut().action_set(action));
                },
                _ => {
                    return;
                }
            }
        }
        game.tick();
        match game.player {
            Some(ref pl) => self.render_controller.set_player_pos(&*pl.borrow()),
            None => {}
        }
    }

    pub fn run (&mut self, window : &mut Window, game : &mut GameState) {
        let event_settings = EventSettings {
            updates_per_second: 60,
            max_frames_per_second: 60,
        };

        game.update_player_los();
        {
            let ref pl = game.player.as_ref();
            if pl.is_some() {
                let pl = pl.unwrap();

                self.render_controller.set_player_pos(&*pl.borrow());
                self.render_controller.move_camera_to_destination();

                let &PistonUI {
                    ref mut renderer,
                    ref mut render_controller,
                    ..
                } = self;

                render_controller.update_camera(renderer);
            }
        }

        let mut render_time = time::precise_time_ns();

        let mut events = EventIterator::new(window, &event_settings);
        for e in events {
            match e {
                Render(_) => {
                    let &PistonUI {
                        ref mut renderer,
                        ref mut render_controller,
                        ..
                    } = self;

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
                    self.game_update(game);
                },
                Input(i) => {
                    self.input_controller.push_input(i);
                    self.game_update(game);
                }
            }
        }
    }

}
