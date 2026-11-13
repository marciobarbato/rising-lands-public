//! Declared player input. All answers are owned values; there are no host callbacks.
use crate::{
    frame::{stop, FrameEvent, FrameInput},
    EngineError, MemoriaJogo, Result,
};
use std::collections::BTreeMap;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerEvent {
    pub kind: u32,
    pub phase: u32,
    pub site: u32,
    pub frame: u32,
    pub values: [u32; 12],
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclaredInput {
    pub cursor: Option<(i32, i32)>,
    pub messages: Vec<[u32; 3]>,
    /// Signed schema answers are retained as their original 32-bit representation.
    pub keys: BTreeMap<u32, u32>,
    /// Explicit Missions display-scenario TextGetMessage boundary, never a default.
    pub display_text_boundary: bool,
    /// Missions explicitly declares these original sound roots non-executed.
    pub display_sound_boundary: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Runtime {
    pub answers: DeclaredInput,
    pub events: Vec<PlayerEvent>,
    pub phase: u32,
    pub frame: u32,
}
pub trait Memory {
    fn b(&self, a: u32) -> Result<u32>;
    fn w(&self, a: u32) -> Result<u32>;
    fn d(&self, a: u32) -> Result<u32>;
    fn sb(&mut self, a: u32, v: u32) -> Result<()>;
    fn sw(&mut self, a: u32, v: u32) -> Result<()>;
    fn sd(&mut self, a: u32, v: u32) -> Result<()>;
}
impl Memory for MemoriaJogo {
    fn b(&self, a: u32) -> Result<u32> {
        Ok(u32::from(self.u8(a)?))
    }
    fn w(&self, a: u32) -> Result<u32> {
        Ok(u32::from(self.u16(a)?))
    }
    fn d(&self, a: u32) -> Result<u32> {
        self.u32(a)
    }
    fn sb(&mut self, a: u32, v: u32) -> Result<()> {
        self.set_u8(a, v as u8)
    }
    fn sw(&mut self, a: u32, v: u32) -> Result<()> {
        self.set_u16(a, v as u16)
    }
    fn sd(&mut self, a: u32, v: u32) -> Result<()> {
        self.set_u32(a, v)
    }
}
pub fn event(m: &mut MemoriaJogo, kind: u32, site: u32, values: &[u32]) {
    if !m.audit && kind < 7 {
        return;
    }
    if let Some(r) = m.player.as_mut() {
        let mut e = PlayerEvent {
            kind,
            phase: r.phase,
            site,
            frame: r.frame,
            ..PlayerEvent::default()
        };
        e.values[..values.len()].copy_from_slice(values);
        r.events.push(e);
    }
}
pub fn key(m: &mut MemoriaJogo, vk: u32, site: u32) -> Result<i32> {
    let value = m
        .player
        .as_ref()
        .ok_or(EngineError::UnportedCallee {
            name: "declared GetAsyncKeyState",
            address: site,
        })?
        .answers
        .keys
        .get(&vk)
        .copied()
        .unwrap_or(0);
    event(m, 6, site, &[vk, value]);
    Ok(value as u16 as i16 as i32)
}
pub fn unit(m: &MemoriaJogo, u: u32) -> Result<u32> {
    Ok(m.d(0x4e19a0)?.wrapping_add((u & 65535) * 276))
}
pub fn structure(s: u32) -> u32 {
    0x49a0c8 + (s & 65535) * 276
}
pub fn alive(m: &MemoriaJogo, u: u32) -> Result<bool> {
    Ok(!matches!(m.b(unit(m, u)? + 2)?, 0 | 4))
}
pub fn selections(m: &MemoriaJogo, p: u32) -> Result<Vec<u32>> {
    let a = 0x508728 + ((p & 255) << 9);
    (0..m.w(a)?).map(|i| m.w(a + 2 + 2 * i)).collect()
}
pub fn count(m: &MemoriaJogo, p: u32) -> Result<u32> {
    m.w(0x508728 + ((p & 255) << 9))
}
pub fn first(m: &MemoriaJogo, p: u32) -> Result<u32> {
    let a = 0x508728 + ((p & 255) << 9);
    if m.w(a)? != 0 {
        m.w(a + 2)
    } else {
        Ok(65535)
    }
}
pub fn selected_structure(m: &MemoriaJogo, p: u32) -> Result<u32> {
    m.w(0x495be0 + 2 * (p & 255))
}
pub fn text(m: &mut MemoriaJogo, n: u32, site: u32) -> Result<u32> {
    let p = m.d(0x5099a4)?;
    if p != 0 {
        return crate::text::get(m, n);
    }
    if m.player
        .as_ref()
        .is_some_and(|r| r.answers.display_text_boundary)
    {
        event(m, 9, site, &[n & 65535, 0]);
        return Ok(0);
    }
    stop(
        "TextGetMessage with the text table pointer 0x5099a4 = 0 (texts not loaded)",
        0x478a47,
    )
}
pub fn sound(
    m: &mut MemoriaJogo,
    site: u32,
    post: bool,
    r: [u32; 4],
    volume: Option<u32>,
) -> Result<u32> {
    let mut v = vec![
        u32::from(post),
        r[0],
        r[1],
        r[2],
        r[3],
        u32::from(volume.is_some()),
    ];
    v.push(volume.unwrap_or(0));
    event(m, 10, site, &v);
    if m.player
        .as_ref()
        .is_some_and(|runtime| runtime.answers.display_sound_boundary)
    {
        return Ok(r[0]);
    }
    crate::player_audio::request(m, post, r)
}
pub fn order(m: &mut MemoriaJogo, p: u32, tag: u32, words: [u32; 4], site: u32) -> Result<u32> {
    let a = m.d(0x4e2458 + 4 * (p & 255))?;
    m.sw(a, tag)?;
    for (i, v) in words.iter().enumerate() {
        m.sw(a + 2 + 2 * i as u32, *v)?;
    }
    event(
        m,
        7,
        site,
        &[
            p & 255,
            tag,
            words[0] & 65535,
            words[1] & 65535,
            words[2],
            words[3],
        ],
    );
    Ok(a)
}
pub fn phase(
    m: &mut MemoriaJogo,
    phase: u32,
    mut input: FrameInput,
    answers: DeclaredInput,
) -> Result<(FrameInput, Vec<FrameEvent>, Vec<PlayerEvent>)> {
    if !matches!(phase, 2 | 3) {
        return Err(EngineError::InvalidFramePhase(phase));
    }
    let frame = m.d(0x508f7c)?.wrapping_add(u32::from(phase == 2));
    let xy = answers
        .cursor
        .unwrap_or((m.w(0x5099a8)? as i32, m.w(0x5099aa)? as i32));
    input.cursor_x = xy.0;
    input.cursor_y = xy.1;
    m.player = Some(Runtime {
        answers,
        events: Vec::new(),
        phase,
        frame,
    });
    let result = crate::frame::phase(m, phase, input);
    match result {
        Ok((out, events, _)) => {
            let runtime = m.player.take().expect("declared input runtime");
            Ok((out, events, runtime.events))
        }
        Err(e) => Err(e),
    }
}

/// Captured text requests from the offline fixture transaction, in call order.
pub fn fixture_text_requests(m: &MemoriaJogo) -> Vec<[u32; 3]> {
    m.player
        .as_ref()
        .into_iter()
        .flat_map(|r| r.events.iter())
        .filter(|e| e.kind == 9)
        .map(|e| [e.site, e.values[0], e.values[1]])
        .collect()
}

/// The kind-10 sound requests a fixture published: call site, then post, the
/// four request registers, the volume flag and the volume (`sound`).
pub fn fixture_sound_requests(m: &MemoriaJogo) -> Vec<[u32; 8]> {
    m.player
        .as_ref()
        .into_iter()
        .flat_map(|r| r.events.iter())
        .filter(|e| e.kind == 10)
        .map(|e| {
            let mut v = [e.site, 0, 0, 0, 0, 0, 0, 0];
            v[1..].copy_from_slice(&e.values[..7]);
            v
        })
        .collect()
}

/// Offline routine adapter. It supplies exactly the declared answer table and
/// exposes semantic returns only; register leftovers are not invented.
pub fn fixture(
    m: &mut MemoriaJogo,
    name: &str,
    a: [u32; 6],
    answers: DeclaredInput,
) -> Result<Option<u32>> {
    m.player = Some(Runtime {
        answers,
        ..Runtime::default()
    });
    let [ax, dx, bx, cx, s0, s1] = a;
    let out = match name {
        "AvanceMonstre" => {
            crate::frame_raid::advance(m)?;
            None
        }
        "AnimPorteBatiment" => {
            crate::building_support::anim_porte(m, ax as u16)?;
            None
        }
        "EnginNextEtape" => {
            crate::frame_units::call(m, "EnginNextEtape", &[ax & 65535])?;
            None
        }
        "DisplayEffects" => {
            crate::frame_input::display_effects(m, ax)?;
            None
        }
        "AffGameDisplayEffects" => {
            crate::frame_input::display_effects_for(m, ax, true)?;
            None
        }
        "PlaySnd_PostSound" => Some(crate::player_audio::request(m, true, [ax, dx, bx, cx])?),
        "PlaySnd_StopSound" => Some(crate::player_audio::request(m, false, [ax, dx, bx, cx])?),
        "ResetOrdre" => {
            crate::frame_spawn::reset_order(m, ax & 65535)?;
            Some(unit(m, ax)?)
        }
        "SetDeplaceEngin" => {
            crate::frame_spawn::order(m, ax & 65535, dx as u16)?;
            Some(unit(m, ax)?)
        }
        "DeplaceEngin" => {
            crate::frame_movement::displace(m, ax & 65535)?;
            None
        }
        "EnginOccupeCase" => Some(u32::from(crate::player_build::occupies_mode(
            m,
            ax & 65535,
            dx & 65535,
            bx & 255,
        )?)),
        "BatimentCollision" => Some(crate::frame_core::call(m, "BatimentCollision", &[ax, dx])?),
        "TerrainEstHerbe" => {
            let t = m.w(m.d(0x507d70)? + 2 * (ax & 65535))?;
            Some(u32::from((0x15d..0x1a5).contains(&t)))
        }
        "ValideHerse" => Some(crate::player_build::gate(m, ax)?),
        "ChercheMurCombinaisons" => Some(crate::player_build::combination(m, ax)?),
        "ResetCheminTerrainModify" => Some(crate::player_build::reset_terrain(m, ax & 65535)?),
        "InputGame" => {
            crate::player_input::input(m, ax & 255)?;
            None
        }
        "CalculIconeAction" => {
            let v = crate::player_input::icons(m)?;
            if count(m, m.b(0x508041)?)? == 1 || selected_structure(m, m.b(0x508041)?)? != 65535 {
                None
            } else {
                Some(v)
            }
        }
        "ProcessInputGame" => Some(crate::player_orders::process(m, ax)?),
        "CalculWorkEngin" => Some(crate::chain_work::work(m, ax & 65535)?),
        "WndProcF11" => Some(crate::player_input::wnd(m, [0x100, 0x7a, 0])?.0),
        "EnginCalculTransporte" => {
            crate::transport_delivery_next::worker(m, ax as u16)?;
            Some(0)
        }
        "CalculDechargeCaveEngin" => {
            crate::transport_delivery_next::cave_unload(m, ax as u16)?;
            Some(0)
        }
        "CalculAttaqueEngin" => {
            crate::chain_attack::approach(m, ax as u16)?;
            Some(0)
        }
        "InitBatimentNeutre" => Some(
            crate::chain_habitation::own(
                m,
                "InitBatimentNeutre",
                crate::FrameRegisters {
                    eax: ax,
                    edx: dx,
                    ..Default::default()
                },
            )?
            .eax,
        ),
        "BatimentNextEtape" => Some(crate::frame_units::call(
            m,
            "BatimentNextEtape",
            &[ax & 65535],
        )?),
        "EnginNextCase" => Some(crate::frame_movement::call(
            m,
            "EnginNextCase",
            &[ax & 65535],
        )?),
        "iagUnitDeleteAllGroups" => {
            crate::mission_ai::delete_all_groups(m, ax)?;
            None
        }
        "CalculNextReflect" => {
            crate::frame_ai::call(m, "CalculNextReflect", &[ax & 255, dx & 255])?;
            None
        }
        "SetPlayerOrdre" => {
            order(m, ax, dx, [bx, cx, s0, s1], 0x45ecd4)?;
            Some(m.d(0x4e2458 + 4 * (ax & 255))?)
        }
        "MouseZone" => Some(crate::player_input::zone(m)?),
        "MouseEstDeplacer" => {
            let mut v = 0;
            for (old, new) in [(0x49a042, 0x5099a8), (0x49a044, 0x5099aa)] {
                let (a, b) = (m.w(old)?, m.w(new)?);
                v = a.min(b) & 0xff00;
                if a.max(b) - a.min(b) > 10 {
                    v |= 1;
                    break;
                }
            }
            Some(v)
        }
        "KeyPressed" => {
            let vk = ax & 255;
            let r = key(m, vk, 0x42c348)?;
            if r < 0 {
                m.sb(0x4e0630 + vk, 1)?;
                Some((r as u32 & 0xffff0000) | 0x101)
            } else {
                Some(r as u32 & !255)
            }
        }
        "KeyUnPressed" => {
            let vk = ax & 255;
            if m.b(0x4e0630 + vk)? != 1 {
                Some(0)
            } else {
                let r = key(m, vk, 0x42c37d)?;
                if r < 0 {
                    Some(r as u32 & !255)
                } else {
                    m.sb(0x4e0630 + vk, 0)?;
                    Some((r as u32 & 0xffff0000) | 1)
                }
            }
        }
        "KeyTestRelease" => {
            let r = key(m, ax & 255, 0x42c3a7)?;
            if r < 0 {
                return stop(
                    "KeyTestRelease held declared answer does not release",
                    0x42c3a7,
                );
            }
            Some(r as u32)
        }
        "WndProc" => {
            crate::player_input::wnd(m, [dx, bx, cx])?;
            Some(0)
        }
        "ReseauActif" => Some((ax & !255) | m.b(0x4e0730)?),
        "GrabAllInput" => {
            let (x, y) = m.player.as_ref().and_then(|r| r.answers.cursor).ok_or(
                EngineError::UnportedCallee {
                    name: "declared GetCursorPos",
                    address: 0x474fee,
                },
            )?;
            m.sw(0x5099a8, x as u32)?;
            m.sw(0x5099aa, y as u32)?;
            Some(y as u32)
        }
        "InitInputPlayer" => return stop("GlobalAlloc", 0x467ec0),
        "GoLastAttaque" => {
            crate::player_input::last_attack(m, ax, dx)?;
            None
        }
        "MiniMapGetCase" => Some(crate::player_input::minimap_case(m, ax, dx)?),
        "MiniMapGetCenterSelect" => {
            crate::player_input::minimap_center(m, ax, dx, bx, cx)?;
            None
        }
        "MiniMapSetLock" => {
            m.sw(0x507d6c, m.w(0x507d68)?.wrapping_sub(ax & 65535))?;
            m.sw(0x507d6e, m.w(0x507d6a)?.wrapping_sub(dx & 65535))?;
            None
        }
        "IconeGetZone" => Some(crate::player_input::icon_zone(m, ax, dx)?),
        "EnginGetBagOrdre" => Some(crate::player_input::bag(m, ax)?),
        "BagGetCode" => Some((ax & !255) | m.b(ax.wrapping_add(0x42 + (dx & 65535)))?),
        "BagGetType" => Some((ax & 0xffff0000) | m.w(ax.wrapping_add(6 + 2 * (dx & 65535)))?),
        "BatimentPosable" => Some(crate::player_orders::posable(
            m,
            ax & 255,
            dx & 65535,
            bx & 65535,
        )?),
        "BatimentConstructible" => Some(u32::from(crate::player_build::constructible(
            m,
            ax & 255,
            dx & 65535,
            bx & 65535,
        )?)),
        "PlaceBatiment" => {
            crate::player_build::place(m, ax & 65535)?;
            None
        }
        "CreerBatiment" => Some(crate::player_build::create(m, ax, dx & 255, bx & 255)?),
        "CalculConstruireBatiment" | "EnginNextEtape:state11" => {
            crate::player_build::builder(m, ax & 65535)?;
            None
        }
        "DecouvreTerrain" | "IllumineTerrain" | "EteintTerrain" => {
            crate::player_sight::area(
                m,
                ax as u16,
                dx & 65535,
                bx as u8,
                match name {
                    "DecouvreTerrain" => 0,
                    "IllumineTerrain" => 1,
                    _ => 2,
                },
            )?;
            None
        }
        "SetMiniMap" => Some(crate::player_sight::minimap(
            m, ax as u8, bx as u16, dx as u8,
        )?),
        "SelectionAddEngin"
        | "RetireSelectionEngin"
        | "EnginSelectionnable"
        | "EnginInZone"
        | "DeselectionEngin"
        | "Deselection"
        | "SetRecolteEngin"
        | "SetTransporteEngin" => {
            let name = match name {
                "SelectionAddEngin" => "SelectionAddEngin",
                "RetireSelectionEngin" => "RetireSelectionEngin",
                "EnginSelectionnable" => "EnginSelectionnable",
                "EnginInZone" => "EnginInZone",
                "DeselectionEngin" => "DeselectionEngin",
                "Deselection" => "Deselection",
                "SetRecolteEngin" => "SetRecolteEngin",
                _ => "SetTransporteEngin",
            };
            crate::player_orders::call(m, name, [ax, dx, bx, cx], s0)?;
            None
        }
        "SelectionEngin"
        | "DeSelectionGroupe"
        | "BatimentDeselectionner"
        | "BatimentSelection"
        | "EnginAddSelection"
        | "EnginAsk"
        | "SetDeplaceEnginSelect"
        | "SetAttaqueEnginSelect"
        | "SetConstruitBatimentSelect"
        | "SetEnginConstruitBatiment"
        | "SetRecolteEnginSelect"
        | "SetTransporteEnginSelect"
        | "SetMineEnginSelect"
        | "SetRamasseEnginSelect"
        | "SetDepotEnginSelect"
        | "EnginTravailSelectionne" => {
            let name = match name {
                "SelectionEngin" => "SelectionEngin",
                "DeSelectionGroupe" => "DeSelectionGroupe",
                "BatimentDeselectionner" => "BatimentDeselectionner",
                "BatimentSelection" => "BatimentSelection",
                "EnginAddSelection" => "EnginAddSelection",
                "EnginAsk" => "EnginAsk",
                "SetDeplaceEnginSelect" => "SetDeplaceEnginSelect",
                "SetAttaqueEnginSelect" => "SetAttaqueEnginSelect",
                "SetConstruitBatimentSelect" => "SetConstruitBatimentSelect",
                "SetEnginConstruitBatiment" => "SetEnginConstruitBatiment",
                "SetRecolteEnginSelect" => "SetRecolteEnginSelect",
                "SetTransporteEnginSelect" => "SetTransporteEnginSelect",
                "SetMineEnginSelect" => "SetMineEnginSelect",
                "SetRamasseEnginSelect" => "SetRamasseEnginSelect",
                "SetDepotEnginSelect" => "SetDepotEnginSelect",
                _ => "EnginTravailSelectionne",
            };
            let v = crate::player_orders::call(m, name, [ax, dx, bx, cx], 0)?;
            if name == "EnginTravailSelectionne" {
                Some(v)
            } else {
                None
            }
        }
        "BatimentLogeEngin" => Some(u32::from(crate::chain_entry::lodge(
            m, ax as u16, dx as u16,
        )?)),
        "CalculCompteurMarche" => {
            crate::market::counter_market(m, ax)?;
            None
        }
        "CalculCompteurProposition" => {
            crate::market::counter_proposal(m, ax)?;
            None
        }
        "CalculMarche" => {
            crate::market::calcul_marche(m, ax)?;
            None
        }
        "MarcheDisplayTick" => {
            crate::market::display_tick(m)?;
            None
        }
        "MarcheSetPos" => {
            crate::market::set_pos(m, ax, dx)?;
            None
        }
        "EnginSelectPeutAttaquer"
        | "EnginSelectEstTroupe"
        | "EnginSelectEstEngin"
        | "EnginSelectContientMiroir"
        | "EnginSelectPeuventTransporter"
        | "EnginSelectEstBlesse"
        | "EnginSelectPeutDeposer"
        | "BatimentEstDepot"
        | "BatimentEstNeutre"
        | "BatimentEstEnnemi"
        | "PlayerEstAllie"
        | "BatimentPeutSeVendre"
        | "ChercheIndiceBatiment" => {
            Some(crate::player_input::selection_leaf(m, name, ax, dx, bx)?)
        }
        // One routine of the CalculGame frame body (frame::call), with the
        // play-mode player runtime the body runs under: "frame:NAME".
        _ if name.starts_with("frame:") => Some(crate::frame::call(m, &name[6..], &[ax, dx])?),
        _ => return stop("unported player routine fixture", 0x429278),
    };
    Ok(out)
}
