//! Semantic pilot state and an explicit original-address projection.
pub mod ai_group_fill;
pub mod building_gate;
pub mod building_loop_completion;
pub mod building_loop_tails;
pub mod building_support;
pub mod building_type20;
pub mod chain_attack;
pub mod chain_cargo;
pub mod chain_damage;
pub mod chain_death;
pub mod chain_unblock;
pub mod hospital_structure_chain;
pub mod input_hover_undiscovered;
mod mcicd_events;
pub mod monster_advance;
pub mod palette_tables;
pub mod spell_effects;
pub mod spell_projectile_lifecycle;
pub mod spell_projectiles;
pub mod spell_queries;
pub mod spell_range;
pub mod spell_remaining;
pub mod stage_unit_progression;
pub mod terrain_gc;
pub mod terrain_gc_hover;
pub mod terrain_mine;
pub mod turret_structure_chain;
pub mod vision_sight_boundaries;
mod clock {
    include!("clock.rs");
    include!("save_clock.rs");
}
mod damaged_structure_render;
mod dismantle_scene;
pub mod display_stage_domain;
mod facade37a_modes;
pub mod frame;
pub mod frame_combat;
pub mod frame_runtime_state;
pub mod frame_tasks;
pub mod game_hud_background;
pub mod render;
pub mod render_assets;
mod render_event;
mod render_hud;
mod render_market;
mod render_scene;
mod render_scripted;
mod render_text;
mod render_units;
pub mod research_building;
pub mod research_progress;
pub mod stage_objects;
pub mod structure_overlay_render;
pub mod unit_display_extra;
pub mod unit_experience_render;
mod vehicle_repair_render;
pub use frame_runtime_state::RuntimeState;
pub(crate) mod affflamme_table;
pub mod ai_attack_aim;
pub mod ai_behaviour_battle;
pub mod ai_building_attack;
pub mod ai_building_cell;
pub mod ai_building_fire_chain;
pub mod ai_building_room;
pub mod ai_city_message;
pub mod ai_field_search;
pub mod ai_free_place;
pub mod ai_group_message;
pub mod ai_kernels;
pub mod ai_occupied_place;
pub mod ai_placement_next;
pub mod ai_recruit_message;
pub mod ai_resource_orders;
pub mod ai_rules;
pub mod frame_ai;
pub mod frame_core;
pub mod frame_economy;
pub mod frame_input;
pub mod frame_missions;
pub mod frame_movement;
pub mod frame_path;
pub mod frame_raid;
pub mod frame_spawn;
pub mod frame_state;
mod modern_path;
pub use frame_state::{FrameGlobals, FrameState, StructureState, UnitState};
pub mod building_dismantle;
pub mod frame_units;
mod memory {
    include!("memory.rs");
    include!("save_memory.rs");
}
pub mod save;
mod state;
pub use clock::*;
pub use frame::{FrameEvent, FrameInput, FrameRegisters};
pub use memory::*;
pub use state::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    EmptyRegion,
    AddressOverflow,
    OverlappingRegions,
    UndeclaredAccess {
        address: u32,
        size: usize,
    },
    IncompleteField {
        address: u32,
    },
    ProjectionFieldMismatch {
        address: u32,
    },
    UnsupportedThreadGetter {
        address: u32,
        value: u32,
    },
    UnknownRoutine(u32),
    InvalidTickRate,
    UnportedCallee {
        name: &'static str,
        address: u32,
    },
    InvalidFramePhase(u32),
    InvalidExecutionMode(u32),
    ExecutionReplayMismatch,
    ExecutionProjectionMismatch,
    /// A rule bit outside `RULES_KNOWN`.
    InvalidRules(u32),
}
pub type Result<T> = std::result::Result<T, EngineError>;
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnportedCallee { name, address } => write!(f, "{name} at 0x{address:08x}"),
            _ => write!(f, "{self:?}"),
        }
    }
}
impl std::error::Error for EngineError {}

pub mod message_box;
pub mod messagebox_state;
pub mod options_screen;
pub mod player;
mod player_audio;
mod player_build;
mod player_input;
mod player_loop;
mod player_orders;
mod player_sight;
pub(crate) mod player_spy;
pub mod shell;
pub mod shell_animation;
pub mod shell_briefing;
pub mod shell_draw;
pub mod shell_input;
pub mod shell_menu;
pub mod shell_picture;
pub mod shell_state;
pub mod text;
pub(crate) mod unit_park;

mod play;

mod chain_bags;
mod chain_display_origins;
mod chain_fatigue;
mod chain_queries;
mod chain_recruit;
mod chain_repair;
mod chain_restore;
mod chain_s1;
mod chain_search;
pub mod chain_technology;
mod chain_turret;
mod chain_work;
mod market;

mod chain_entry;
pub mod chain_event_path;
mod chain_habitation;
pub mod chain_stage5;
pub mod chain_unload;
mod chain_vision;
pub mod event_flc;
pub mod events_host;
mod player_cave;

mod chain_battle;
mod chain_groups;
mod chain_population;

mod render_chain;

pub mod campaign_flow;
pub mod campaign_session;
pub mod difficulty;
pub mod mission_ai;
mod mission_aux;
pub mod mission_boot;
pub mod mission_campaign;
pub mod mission_entry;
pub mod mission_files;
pub mod mission_ini;
pub mod mission_load;
pub mod mission_research;
pub mod mission_startup;
mod mission_tail;
pub mod mission_terrain;

pub mod projectile_occupied_impact;

pub mod load_tail_input;
pub mod projectile_building_impact;

pub mod transport_delivery_next;
pub mod transport_unit_render;

#[cfg(feature = "validation-trace")]
pub fn turret_helpers() -> turret_structure_chain::Helpers {
    turret_structure_chain::Helpers {
        enemy_unit: chain_battle::enemy_unit,
        angle: chain_attack::angle,
    }
}

pub mod ai_stage25;
pub mod ai_stage25_ghost_chain;
pub mod ai_stage25_reserve;

pub mod ai_war_build;
pub mod ai_war_enemy;
pub mod ai_war_position;

pub mod spell_order_acceptance;

pub mod order_queue;
pub use order_queue::{OrderQueue, RULE_ORDER_QUEUE};

/// SetDeplaceEnginSelect as ProcessInputGame's move order (0x45efd8) issues it:
/// every live unit of `player`'s current selection list goes to `cell` (and,
/// under the modern rules, each gets its own goal around it). For tools/tests.
pub fn order_selection_move(m: &mut MemoriaJogo, player: u8, cell: u16) -> Result<u32> {
    let p = u32::from(player);
    player_orders::call(
        m,
        "SetDeplaceEnginSelect",
        [p, u32::from(cell), p << 2, 0],
        0x45efd8,
    )
}

#[cfg(feature = "validation-trace")]
pub fn spell_order_explicit_input(m: &mut MemoriaJogo, owner: u32, costs: [u16; 3]) -> Result<()> {
    m.player = Some(player::Runtime::default());
    player_input::input_with_spell_costs(m, owner, costs)
}
#[cfg(feature = "validation-trace")]
pub fn spell_order_panel_context(m: &mut MemoriaJogo, owner: u32, costs: [u16; 3]) -> Result<()> {
    m.player = Some(player::Runtime::default());
    player_input::spell_panel_fragment(m, owner, costs)
}
#[cfg(feature = "validation-trace")]
pub fn spell_order_unit_context(m: &mut MemoriaJogo, unit: u32) -> Result<()> {
    m.player = Some(player::Runtime::default());
    frame::call(m, "EnginNextEtape", &[unit]).map(|_| ())
}
