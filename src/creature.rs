// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

use ai::{Actor,AIActor};
use game;
use game::Action;
use game::{Melee,Turn,Move,Run,Wait,Use};
use hex2d;
use hex2d::{Left,Right,Forward,Backward};
use hex2d::{Point,Position};
use hex2d::AbsoluteDirection;
use hex2d::Direction;
use map;
use map::Map;
use map::TileType;
use time;

/// Race of the creature
#[deriving(PartialEq)]
#[deriving(Eq)]
pub enum Race {
    Human,
    Scout,
    Grunt,
    Heavy,
}

impl Race {
    pub fn max_health(&self) -> uint {
        match *self {
            Human => 4,
            Scout => 2,
            Grunt => 4,
            Heavy => 8,
        }
    }
    pub fn damage(&self) -> uint {
        match *self {
            Human => 2,
            Scout => 1,
            Grunt => 2,
            Heavy => 3,
        }
    }
}

pub struct CreatureState {
    pub visible: hex2d::Map<bool>,
    pub known: hex2d::Map<bool>,

    pub is_player : bool,

    action_cur : Option<game::Action>,
    action_prev : Option<game::Action>,
    action_pre : bool,
    action_delay : uint,
    action_total_delay : uint,
    last_hit_ns: u64,
    last_attack_ns: u64,
    death_ns: u64,

    pub race : Race,
    health: int,
    alive : bool,
    damage : int,
    pub pos : Position,
    pub pos_prev : Position,
    pos_tiletype : TileType,
}

pub struct Creature {
    state : CreatureState,
    actor : Box<Actor+'static>,
}

impl Creature {
    pub fn new(map : &map::Map, pos : Position, player : bool, race : Race) -> Creature {
        Creature {
            state: CreatureState::new(map, pos, player, race),
            actor: box AIActor::new(),
        }
    }

    pub fn is_player(&self) -> bool {
        self.state.is_player
    }

    pub fn race(&self) -> Race {
        self.state.race
    }

    #[allow(dead_code)]
    pub fn health(&self) -> uint {
        let health = self.state.health;

        if health < 0 {
            return 0u;
        }

        return health as uint;
    }

    #[allow(dead_code)]
    pub fn max_health(&self) -> uint {
        let health = self.state.race.max_health();

        return health as uint;
    }


    pub fn p<'a>(&'a self) -> &'a Point {
        &self.state.pos.p
    }

    pub fn pos<'a>(&'a self) -> &'a Position {
        &self.state.pos
    }

    #[allow(dead_code)]
    pub fn pos_prev<'a>(&'a self) -> &'a Position {
        &self.state.pos_prev
    }

    #[allow(dead_code)]
    pub fn is_turning_rel(&self) -> Option<Direction> {
        match self.state.action_cur {
            Some(Turn(dir)) => Some(dir),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn is_moving_rel(&self) -> Option<Direction> {
        match self.state.action_cur {
            None => None,
            Some(action) => match action {
                Run(rdir)|Move(rdir)|Melee(rdir) => Some(rdir),
                _ => None
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_pre_action(&self) -> bool {
        self.state.action_pre
    }

    pub fn knows(&self, p : Point) ->  bool {
        *self.state.known.at(p)
    }

    pub fn sees(&self, p : Point) ->  bool {
        *self.state.visible.at(p)
    }

    pub fn pos_set(&mut self, map : &Map, pos : Position) {
        self.state.pos = pos;
        self.state.pos_tiletype = map.at(pos.p).tiletype;

        self.update_los(map);
    }

    pub fn pos_prev_set(&mut self, _ : &Map, pos : Position) {
        self.state.pos_prev = pos;
    }

    pub fn action_set(&mut self, action : Action) {
        self.state.action_set(action);
    }

    pub fn tick(&mut self) -> Option<Action> {
        self.state.tick()
    }

    pub fn action_done(&mut self) {
        self.state.action_done()
    }

    pub fn was_attacked_ns(&self) -> u64 {
        self.state.last_hit_ns
    }

    #[allow(dead_code)]
    pub fn has_attacked_ns(&self) -> u64 {
        self.state.last_attack_ns
    }

    pub fn death_ns(&self) -> u64 {
        self.state.death_ns
    }

    pub fn needs_action(&self) -> bool {
        self.state.action_delay == 0 && self.state.action_cur.is_none()
    }

    // Very hacky, recursive LoS algorithm
    fn do_los(
        &mut self,
        map : &map::Map,
        p: hex2d::Point, main_dir : hex2d::AbsoluteDirection,
        dir : Option<hex2d::AbsoluteDirection>,
        pdir : Option<hex2d::AbsoluteDirection>,
        light: int
        ) {

        self.mark_visible(map, p);

        let mut light = light;

        let opaqueness = map.at(p).opaqueness();
        light = light - opaqueness as int;

        if light < 0 {
            return;
        }

        let neighbors = match (dir, pdir) {
            (Some(dir), Some(pdir)) => {
                if dir == pdir {
                    vec!(dir)
                } else {
                    vec!(dir, pdir)
                }
            },
            (Some(dir), None) => {
                if main_dir == dir {
                    vec!(dir, dir + Left, dir + Right)
                } else {
                    vec!(dir, main_dir)
                }
            },
            _ => {
                vec!(main_dir, main_dir + Left, main_dir + Right)
            }
        };

        for &d in neighbors.iter() {
            let n = map.wrap(p + d);
            match dir {
                Some(_) => {
                    self.do_los(map, n, d, Some(d), dir, light);
                },
                None => {
                    self.do_los(map, n, main_dir, Some(d), dir, light);
                }
            };
        }
    }

    pub fn update_los(&mut self, map : &Map) {
        self.forget_visible(map);
        for &p in self.p().neighbors().iter() {
            let p = map.wrap(p);
            self.mark_known(map, p);
        }
        let p = self.state.pos.p;
        let dir = self.state.pos.dir;
        self.do_los(map,p, dir, None, None, 15);
    }

    fn mark_known(&mut self, map : &Map, p : hex2d::Point) {
        self.state.mark_known(map, p);
    }

    fn mark_visible(&mut self, map : &Map, p : hex2d::Point) {
        self.state.mark_visible(map, p);

        let Creature {
            ref mut actor,
            ..
        } = *self;

        actor.proceed_visible(map, p);
    }


    pub fn forget_visible(&mut self, map : &Map) {
        self.state.forget_visible(map);
    }

    /// This creature has been attacked some other creature
    pub fn attacked_by(&mut self, cr : &Creature) {
        self.state.last_hit_ns = time::precise_time_ns();
        self.state.health = self.state.health - cr.state.damage;
        if self.state.health <= 0 {
            self.die();
        }
    }

    /// This creature has attacked some other creature
    pub fn attacked(&mut self, _ : &Creature) {
        self.state.last_attack_ns = time::precise_time_ns();
    }

    fn die(&mut self) {
        self.state.death_ns = time::precise_time_ns();
        self.state.alive = false;
    }

    pub fn is_alive(&self) -> bool {
        self.state.alive
    }

    pub fn update_action(&mut self, map : &map::Map) {

        let Creature {
            ref mut state,
            ref mut actor,
            ..
        } = *self;

        let action = actor.get_action(map, state);
        state.action_set(action);
    }
}

impl CreatureState {
    pub fn new(map : &map::Map, pos : Position, is_player : bool, race : Race) -> CreatureState {
        CreatureState {
            visible: map.clone(false),
            known: map.clone(false),
            action_pre: true,
            action_cur: None,
            action_prev: None,
            action_total_delay: 0,
            action_delay: 0,
            is_player: is_player,
            race: race,
            health: race.max_health() as int,
            damage: race.damage() as int,
            alive: true,
            pos: pos,
            pos_prev: pos,
            pos_tiletype: map.at(pos.p).tiletype,
            last_hit_ns: 0,
            last_attack_ns: 0,
            death_ns: 0,
        }
    }

    pub fn action_set(&mut self, action : game::Action) {
        self.action_cur = Some(action);
        self.action_pre = true;

        self.action_total_delay = self.action_delay(action, true);
        self.action_delay = self.action_total_delay;
    }

    pub fn tick(&mut self) -> Option<Action> {
        if self.action_delay > 0 {
            self.action_delay -= 1;
            if self.action_delay == 0 && !self.action_pre {
                self.action_prev = self.action_cur;
                self.action_cur = None;
            }

            return None;
        }

        assert!(self.action_pre);
        let action = self.action_cur.unwrap();
        self.action_pre = false;

        Some(action)
    }

    fn action_done(&mut self) {
        self.action_total_delay = self.action_delay(self.action_cur.unwrap(), false);
        self.action_delay = self.action_total_delay;
        if self.action_delay > 0 {
            self.action_pre = false;
        } else {
            self.action_prev = self.action_cur;
            self.action_cur = None;
        }
    }

    fn action_delay(&self, action : Action, pre : bool) -> uint {
        let delay = match action {
            Run(Forward)|Run(Left)|Run(Right) => match self.action_prev {
                Some(Run(Forward))|Some(Run(Left))|Some(Run(Right)) => 0,
                _ => if pre { 0 } else {1},
            },
            Turn(_) => 0,
            Move(Forward) => if pre { 0 } else { 1 } ,
            Move(Left)|Move(Right) =>  if pre { 0 } else { 1 },
            Run(Backward) | Move(Backward) => 1,
            Melee(_) => if pre { 0 } else { 1 },
            Wait => 0,
            Use => 4,
        };

        /* Terrain modifier */
        match action {
            Run(_)|Move(_) => {
                delay + self.pos_tiletype.move_delay()
            },
            Turn(_) => {
                delay + if pre {self.pos_tiletype.move_delay() } else { 0 }
            },
            _ => delay,
        }
    }

    fn mark_visible(&mut self, _: &Map, p : Point) {
        *self.visible.mut_at(p) = true;
        *self.known.mut_at(p) = true;
    }

    fn mark_known(&mut self, _: &Map, p : Point) {
        *self.known.mut_at(p) = true;
    }

    pub fn forget_visible(&mut self, map : &Map) {
        self.visible = map.clone(false);
    }
}
