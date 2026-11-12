#ifndef RISING_ENGINE_H
#define RISING_ENGINE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* ABI1.6. Frame logic and native indexed rendering are additive.
 * Addresses are original u32 values, NOT host pointers. Handles are opaque
 * integer tokens, never dereferenceable; zero/stale/double-free are rejected.
 * Calls are serialized inside the library. Caller must synchronize handle use
 * with free. No Rust-owned pointers escape; all buffers are copied.
 * All nonnull pointers must be aligned, valid for their declared lengths and
 * disjoint from output storage. No call may race access to caller buffers.
 * Success0; all errors leave world/registers unchanged. Projection query writes
 * address/required metadata on RL_BUFFER_TOO_SMALL; buffer stays unchanged.
 * Load replaces ALL blocks atomically, preserving the explicit virtual clock.
 * Count<=65536 and total<=64MiB. Empty load is allowed; empty regions are not.
 */
#define RL_ABI_VERSION 0x00010006u
enum { RL_OK=0, RL_INVALID_ARGUMENT=1, RL_INVALID_HANDLE=2, RL_MEMORY_ERROR=3,
 RL_UNKNOWN_ROUTINE=4, RL_UNSUPPORTED_PATH=5, RL_BUFFER_TOO_SMALL=6,
 RL_PANIC=7, RL_POISONED=8, RL_RESOURCE_LIMIT=9, RL_VERSION_MISMATCH=10, RL_STALE_GENERATION=11 };
enum { RL_RANDOM=1, RL_LONG_RAND=2, RL_CRT_RAND=3, RL_INT_TO_BCD=4, RL_BCD_TO_INT=5 };
typedef uint64_t RlMundo;
typedef struct { uint32_t address, length; const uint8_t *data; } RlBloco;
typedef struct { uint32_t eax, ebx, ecx, edx, esi, edi, ebp; } RlRegisters;
uint32_t rl_versao(void);
int32_t rl_criar_mundo(uint32_t abi_version, uint32_t ticks_per_second,
 uint32_t epoch_ms, RlMundo *output);
int32_t rl_liberar_mundo(RlMundo world);
int32_t rl_carregar_blocos(RlMundo world, const RlBloco *blocks, uint32_t count);
/* After S0 blocks and before input: original InitTextLangue, language0 (PT-BR).
 * Caller reads bundled INSTALHD/TEXTES.TXT as raw bytes. Atomic; rejects a
 * missing section, unsupported language or an already-loaded table. */
int32_t rl_carregar_textos(RlMundo world, const uint8_t *bytes, uint32_t length,
 uint32_t language);
/* Supply original INSTALHD/EVENTS.INI once after S0 startup. Required before
 * notifications; missing input remains an addressed engine stop. */
int32_t rl_carregar_eventos(RlMundo world, const uint8_t *bytes, uint32_t length);
/* Isolated routine call, NOT a tick. Register map: original data GPRs, with
 * no x86 EIP/ESP/EFLAGS contract. No routine here reads a host clock. */
int32_t rl_executar_rotina(RlMundo world, uint32_t routine, RlRegisters *registers);
int32_t rl_quantidade_blocos(RlMundo world, uint32_t *output);
/* Blocks are sorted by numeric address. Query: buffer=NULL, capacity=0. */
int32_t rl_projetar_bloco(RlMundo world, uint32_t index, uint32_t *address,
 uint8_t *buffer, uint32_t capacity, uint32_t *required);
/* Clock foundation only; advances no game logic. Default pacing policy20Hz
 * (caller explicitly passes20), not a proved historical MONDE01 FPS. */
int32_t rl_avancar_ticks(RlMundo world, uint64_t ticks);
int32_t rl_relogio(RlMundo world, uint64_t *tick, uint32_t *milliseconds);
/* Thread-local error, UTF8+NUL. Does not clear error. NULL/zero size query. */
int32_t rl_erro_mensagem(uint8_t *buffer, uint32_t capacity, uint32_t *required);
/* ABI1.1 additions. Creation also accepts1.0/1.1; existing layouts/signatures retain
 * their1.0 semantics. One caller must serialize a phase plus snapshot/events.
 * Input states are explicitly declared memory blocks, including original tables.
 * Supported scenario: MONDE01 no-key display path. Unported branches fail with
 * RL_UNSUPPORTED_PATH naming the original address. All calls are transactional.
 * Phase1(load_tail/frame0) and3(frame) tick once; phase2(between) does not tick.
 * No human-input protocol, pixels or render surface is returned. */
typedef struct { uint32_t eax,ebx,ecx,edx,esi,edi,ebp,esp; } RlFrameRegisters;
typedef struct {
 int32_t cursor_x,cursor_y; uint32_t offscreen; RlFrameRegisters registers;
 uint32_t saved[3],msg[4];
} RlFrameInput;
/* P2 mission extension version1; ABI1.6 signatures remain compatible.
 * Load gameplay directly from original assets, never captured S0. Host owns
 * the software display (800x600), active session, input and renderer lifecycle.
 * New functions publish a FRESH handle and carried input atomically; failures
 * retain outputs/source world. Free the old world/renderer after success and
 * create a renderer for the new handle from explicit surface/cache/assets.
 * Call rl_autosalvar_inicio_fase(new_world,&input) after successful load tail.
 * All strings are raw length-delimited spans, WITHOUT NUL terminators.
 */
uint32_t rl_missao_versao(void);
typedef struct {
 uint32_t version,allocation_fill,ai_clock_ms,thread_id,ticks_per_second,epoch_ms,
  execution_mode,regras;
 uint32_t iterations[3]; RlFrameInput input;
} RlMissaoInicio;
/* Rule set of a world (RlMissaoInicio.regras, rl_regras). 0 is the original game.
 * Each optional feature defines one RL_REGRA_* bit; an unknown bit is refused
 * with RL_INVALID_ARGUMENT. regras replaces the former reserved word with the
 * same offset and size, so hosts that wrote reserved=0 stay original.
 * Bit 0 = optional modern unit paths ("Caminhos melhores"): a deterministic
 * integer A* (no RNG) for the units a move order or the AI walks, spread goals
 * for group moves, wait-and-replan instead of giving up. Without the bit the
 * original CheminCalcul movement runs unchanged.
 * Bit 1 = optional Shift+click order queue (part of the modern shortcuts): a
 * Shift+click on open ground with units selected, a no-op in the original,
 * queues a move waypoint per unit (player order 40); idle units walk them in
 * order; a plain order replaces them. Without the bit, order 40 is refused. */
#define RL_REGRA_CAMINHOS_MELHORES 0x1u
#define RL_REGRA_FILA_ORDENS 0x2u
/* config.version=1, regras=0 or known RL_REGRA_* bits, fill=0..255 (explicit host allocator bytes),
 * execution_mode=0/1/2, rate>0. ai_clock_ms is the original GetTickCount input
 * for AI Randomize; it is NOT replaced with a wall clock or bot seed.
 * input is the explicit WinMain call context (including virtual ESP). Its
 * cursor and iterations[3] bind prior ProgIteration presentation state;
 * use the actual host context (S0 proof: cursor400,300; counters5,2,1).
 * Returns at MissionLoad success, phase0, without consuming a tick. Run phase1
 * (RL_LOAD_TAIL), then supply the carried phase2 WinMain registers and proceed
 * with phases2,3. Unported load-tail displays can stop without losing the load.
 * Ten RlBloco descriptors, IN ORDER (address ignored): RISING.EXE, RISING.INI,
 * RISING.RUL, RISING.SBL, TABLEAU.COL, COULEUR.MAP, PLATGAME.PIE,
 * MONSTRE.INI, TEXTES.TXT, selected ETA. Combined limit64MiB. No pointer retained.
 * name is MONDExx[variant].ETA (case-insensitive), one of the38 shipped files.
 * Direct stage selection starts fresh carry-over; base09/10/12/17 names select
 * the explicit base ETA bytes. Campaign continuation uses the original variant
 * selection, sympathy and carry-over instead. Caller supplies the matching ETA.
 */
int32_t rl_missao_iniciar(const RlBloco *assets,uint32_t count,
 const uint8_t *name,uint32_t name_length,const RlMissaoInicio *config,
 RlMundo *output,RlFrameInput *input_out);
/* Explicit EVENTS host input v1, additive to ABI1.6. This is a host namespace,
 * not a game rule. Logical original absolute Windows drive directory (4..244
 * ASCII bytes, backslashes, no dot segments/aliases/trailing space or dot).
 * Engine opens no host filename. Host maps exactly directory+\EVENTS.INI to
 * its authorized owned physical file and supplies raw bytes (1..1MiB, no NUL).
 * No pointer survives this call: original fields and owned EVENTS projection
 * retain copies. Descriptor size/version must match. No CWD/default/fallback.
 * Fresh entry runs ProgInit474d53..474d9e after permutation initialization, before
 * MissionLoad; it publishes only after the complete original load succeeds.
 * Legacy rl_missao_iniciar / rl_carregar_eventos signatures remain supported;
 * bytes alone DO NOT bind a logical filename. Missing filename remains469a68.
 * Reassociate verifies saved directory, filename and ALL owned file bytes,
 * without rewriting saved memory, generation, tick or path. Old unbound saves
 * and mismatches refuse RL_UNSUPPORTED_PATH; caller discards candidate handle.
 * Restored bytes are engine-owned serialized input; the current external host
 * mapping is asserted afresh by this call, never inferred from installed bytes.
 */
typedef struct {
 uint32_t size,version; const uint8_t *directory; uint32_t directory_length;
 const uint8_t *events; uint32_t events_length;
} RlEventsHostV1;
int32_t rl_missao_iniciar_eventos_v1(const RlBloco *assets,uint32_t count,
 const uint8_t *name,uint32_t name_length,const RlMissaoInicio *config,
 const RlEventsHostV1 *host,RlMundo *output,RlFrameInput *input_out);
/* Program start into the ORIGINAL main menu (mode2) instead of a stage: the
 * first NINE RlBloco descriptors above (no ETA), same config. Runs ProgInit's
 * gameplay portion, the display binding, TEXTES.TXT and MenuInit(local
 * player,2). The intro movie offer of CalculGame mode1 is a host presentation
 * and is not raised. The world has no frame state: do not step it; create a
 * prepared shell on it (rl_shell_create) and drive the original menu. The
 * config execution_mode is NOT applied to the menu world; set it on the stage
 * world returned by rl_shell_campaign_load with rl_execucao_definir before the
 * load tail. The eventos variant binds the EVENTS host as the stage entry does. */
int32_t rl_missao_iniciar_menu(const RlBloco *assets,uint32_t count,
 const RlMissaoInicio *config,RlMundo *output,RlFrameInput *input_out);
int32_t rl_missao_iniciar_menu_eventos_v1(const RlBloco *assets,uint32_t count,
 const RlMissaoInicio *config,const RlEventsHostV1 *host,RlMundo *output,
 RlFrameInput *input_out);
int32_t rl_eventos_reassociar_v1(RlMundo world,const RlEventsHostV1 *host);
/* In-game FLC events, additive (contract: event-flc.md). Declare once per
 * world, before its first frame, every EVENEMENT name of EVENTS.INI, checked
 * against Events\NAME.FLC of the CD root in use:
 * available 0 = the file does not open (original FlicOpen 0: the click does
 * nothing), 1 = header holds the file's first 128 bytes. NAME is 1..15 ASCII
 * alphanumerics, NUL-terminated, unique ignoring case. A clicked notification
 * whose name is outside the table is an addressed stop (0x45a640). Saved with
 * the world. State: running/modal (mode 22) and the table index and header
 * fields of the event in progress. Terminar: the host has shown the last frame;
 * requires a running event in mode 22; the next frames run the original end. */
typedef struct { uint8_t name[16]; uint32_t available; uint8_t header[128]; } RlEventFlcV1;
typedef struct { uint32_t running,modal,index,frames,width,height,speed; } RlEventFlcStateV1;
int32_t rl_eventos_flc_v1(RlMundo world,const RlEventFlcV1 *entries,uint32_t count);
int32_t rl_evento_flc_estado_v1(RlMundo world,RlEventFlcStateV1 *output);
int32_t rl_evento_flc_terminar_v1(RlMundo world);

/* Preview actual original CD-relative path from MissionNext/MakeMissionName.
 * No world/RNG mutation. NULL/zero query returns RL_BUFFER_TOO_SMALL and size;
 * host resolves that path to its bundled WORLD directory (not its host CWD).
 * Check file existence explicitly: after stage25 no next ETA is shipped.
 */
int32_t rl_missao_nome_proxima(RlMundo world,uint8_t *buffer,uint32_t capacity,
 uint32_t *required);
/* Explicit user/host confirmation of victory and between-stage screens.
 * Requires TestFinDeMission end_condition AND victory latch; name must equal
 * the preview path byte-for-byte. Missing/empty ETA is an error. Successful
 * order: MissionNext, AI research adjustment, experience/research/player stores,
 * RechercheCalculNew, load-frame RNG prelude, MissionLoad (returns at phase0).
 * Then run the same load-tail/carried-register protocol as rl_missao_iniciar.
 * Clock and execution mode continue. No input queue is copied from the previous
 * stage; host must consume/discard that queue explicitly before transition.
 * Unported game/UI branches retain their addressed errors (RL_UNSUPPORTED_PATH).
 */
/* Original screen mode, including 19=result. No tick/generation/RNG mutation. */
int32_t rl_missao_tela(RlMundo world,uint32_t *screen);
int32_t rl_missao_continuar(RlMundo world,const uint8_t *name,uint32_t name_length,
 const uint8_t *eta,uint32_t eta_length,const uint8_t *monsters,uint32_t monsters_length,
 const RlFrameInput *input,
 RlMundo *output,RlFrameInput *input_out);

/* saved is ProgIteration's saved EDX/ECX/EBX; msg is its four loop words.
 * registers is the declared phase entry. Phase2 returns next CalculGame entry;
 * phase3 returns preserved registers with ESP+4; EAX is unspecified for phase3.
 * Phase1's register output is unspecified. On failure input is unchanged. */
enum { RL_LOAD_TAIL=1,RL_BETWEEN_FRAMES=2,RL_FRAME=3 };
int32_t rl_passo_quadro(RlMundo world,uint32_t phase,RlFrameInput *input);
typedef struct {
 uint32_t kind,id,site,args[5],result,registers[4],stack[4],stack_mask;
} RlFrameEvent;
/* kind1 binding,2 boundary,3 free; site is original return address. Binding ids:
 *1 ShowCursor,2 GetAsyncKeyState,3 GlobalHandle,4 GlobalUnlock,5 GlobalFree,
 *6 PeekMessageA,7 GetCursorPos,8 SetCursorPos. Boundary ids101 LoadPie,
 *102 MCICD_Play,103 PlaySnd_Manage,104 MCICD_Manage. Registers EAX/EDX/EBX/ECX.
 * stack_mask selects known stack words (uninitialized locals stay unmodeled).
 * MCICD args0/1 carry music guards0x4e2f28/29. Free args0=address,result=size.
 * Unused slots are structural zero. required is record count, not byte count.
 * Successful load/isolated routine clears events; errors retain previous events.
 */
int32_t rl_eventos_quadro(RlMundo world,RlFrameEvent *buffer,uint32_t capacity,uint32_t *required);
/* Packed copied snapshot: LEu32 count then repeated LEu32 address,length,bytes.
 * No alignment padding. NULL/zero query returns RL_BUFFER_TOO_SMALL with
 * required byte count. Other buffer errors never copy a prefix. Freed snapshot
 * contains actual bytes at free, from the last successful phase, sorted by
 * numeric address; it is empty after load or isolated routine calls. */
int32_t rl_projetar_tudo(RlMundo world,uint8_t *buffer,uint32_t capacity,uint32_t *required);
int32_t rl_projetar_liberados(RlMundo world,uint8_t *buffer,uint32_t capacity,uint32_t *required);
/* Isolated provider effects, no tick. Id table: frame_{ai,units,core,economy}
 * ::isolated in rl-core. ABI1.2 adds raid/movement/Combat IDs400..599. Argument counts are exact. Returns only the published
 * semantic EAX/provider value; void adapters return0, no GPR claim. */
int32_t rl_executar_rotina_quadro(RlMundo world,uint32_t routine,const uint32_t *args,uint32_t count,uint32_t *output);
/* ABI 1.3: Graphics render API 1.5, with presentation publication below. Indexed pixels: top-down 800x600,
 * stride 800. Palette: 256 RGB24 entries, static generation 1; no Cyclage.
 * All calls share the world registry lock. Caller serializes render/step/reset/free.
 * Creation copies and verifies nine blobs: BLOCS,TROOPS,VEHICULE,CREATURE,
 * BUILDING,OTHERS CMPs; PLATGAME.PIE; original shadow and lighting tables.
 * Asset payloads are never embedded in the code repository. Initial surface and
 * complete cache are required explicit inputs; no blank/cold defaults.
 * Generation increments on successful load/isolated routine/frame mutations.
 * Render requires phase 1 or 3 (AffGame entry), exact expected generation, and
 * hud_flags 0 or 1. Repeated generation+flags returns the same cached result
 * fields, pixels and palette. RlRenderResult alignment padding is unspecified
 * when a result is published (success or BufferTooSmall); compare all declared
 * fields, including the complete stop arrays, rather than memcmp the struct.
 * Errors other than BufferTooSmall leave the entire result storage unchanged.
 * An accepted consuming draw publishes presentation byte0x4e1d18 to World
 * and its projection, including an accepted partial prefix. It advances neither
 * logic generation nor tick. Other observed writes/RNG effects are diagnostic;
 * their existing phase2 owner remains. World owns a saved generation-scoped
 * consumption receipt across renderers, resets and saves. Replacement draws
 * replay its accepted input without consuming again; logic invalidates it.
 * Modern state schema2 persists the receipt; schema1 is explicitly refused.
 * Success may be full(0), undrawn(1), partial(2). Partial publishes actual prefix
 * and cache, with a named stop. ABI errors publish neither surface nor cache.
 * BufferTooSmall writes result capacities/metadata only; both buffers untouched.
 * Palette equality is pinned; unsupported palette animation is outside 1.5.
 * World free invalidates bound renders/resets/diagnostic reads; renderer free
 * remains possible. Renderer handles are never reused.
 */
typedef uint64_t RlRenderer;
typedef struct { const uint8_t *data; uint32_t length; } RlRenderBytes;
typedef struct { RlRenderBytes keys,payloads; uint32_t cursor; } RlRenderCache;
typedef struct {
 int32_t abi_status; uint32_t disposition; uint64_t generation;
 uint32_t frame_counter,width,height,stride,required_pixels,required_palette;
 uint64_t palette_generation,cache_generation; uint32_t stop_address;
 uint8_t stop_callee[48],stop_reason[192];
} RlRenderResult;
typedef struct {
 uint32_t kind,address; uint8_t name[48]; uint32_t count; int32_t args[8];
 uint8_t before[8],after[8];
} RlRenderObservation;
int32_t rl_render_criar(RlMundo world,const RlRenderBytes *assets,uint32_t count,
 const RlRenderBytes *initial_surface,const RlRenderCache *initial_cache,RlRenderer *output);
/* Player edition, everything from the CD (additive, same ABI version): the
 * renderer from the six CMP banks and PLATGAME.PIE (count 7, rl_render_criar's
 * first seven, in order); the shadow and lighting tables are read from the world
 * the mission start built from the CD palette, and the terrain cache is a new
 * game's (InitCache: keys zero, the bank's blocks 1090..1345, cursor 0). */
int32_t rl_render_criar_cd(RlMundo world,const RlRenderBytes *banks,uint32_t count,
 const RlRenderBytes *initial_surface,RlRenderer *output);
/* rl_render_reset with a new game's terrain cache from the renderer's own bank. */
int32_t rl_render_reset_cd(RlRenderer renderer,const RlRenderBytes *surface);
/* The world's shadow and lighting tables (256 bytes each), computed from the CD palette. */
int32_t rl_mundo_tabelas_sombra_luz(RlMundo world,uint8_t *shadow,uint8_t *light);
/* The world's game palette loaded from PLATGAME.PIE (768 bytes, 8-bit RGB). */
int32_t rl_mundo_paleta_jogo(RlMundo world,uint8_t *palette,uint32_t capacity);
/* A CD shell background (TITRE.PIE, PAGEMIS.PIE, PAGEMIS2.PIE, CREDITS.PIE,
 * ECRANFIN.LZS) decoded to 800x600 indexed pixels (capacity at least 480000). */
int32_t rl_shell_imagem_cd(const uint8_t *file,uint32_t length,uint8_t *pixels,uint32_t capacity);
int32_t rl_render_geracao(RlMundo world,uint64_t *generation,uint32_t *phase,uint32_t *frame);
int32_t rl_render_quadro(RlRenderer renderer,uint64_t expected_generation,uint32_t hud_flags,
 uint8_t *indexed_buffer,uint32_t indexed_capacity,uint8_t *palette_rgb24,uint32_t palette_capacity,
 RlRenderResult *result);
int32_t rl_render_reset(RlRenderer renderer,const RlRenderBytes *surface,const RlRenderCache *cache);
int32_t rl_render_liberar(RlRenderer renderer);
/* Observation kind1: changed bytes (count<=8); kind2: ordered raster arguments
 * (count<=8). name is UTF8+NUL. Reads do not alter last output/cache. */
int32_t rl_render_observacoes(RlRenderer renderer,RlRenderObservation *buffer,uint32_t capacity,uint32_t *required);
int32_t rl_render_cache(RlRenderer renderer,uint8_t *keys,uint32_t key_capacity,
 uint8_t *payloads,uint32_t payload_capacity,uint32_t *key_required,uint32_t *payload_required,uint32_t *cursor);
/* ABI 1.4+: additive Graphics contract 1.9. Existing structs/signatures remain.
 * Observation kind3: write-only saved_background_pixels; count is byte length,
 * full payload comes from rl_render_fundo_salvo (before/after are not old bytes).
 * kind4: AffText request at the named glyph dependency; args hold x,y,right,
 * bottom,font,flags0,flags1,text-length; before+after hold the single-byte text.
 * The production C ABI never substitutes text. AffText returns a partial frame
 * at 0x459454, with actual tooltip background and saved bytes available.
 * Reset clears saved background; a pending initial restore reads the explicitly
 * declared world buffer. Future restores use actual renderer-produced bytes.
 * Phase2 owns other display logic mutations; see presentation publication above.
 */
uint32_t rl_render_contrato(void);
int32_t rl_render_fundo_salvo(RlRenderer renderer,uint8_t *buffer,uint32_t capacity,
 uint32_t *required,uint32_t *address,uint64_t *generation);
/* ABI 1.5: declared per-frame input from monde01-scripted-input/1.
 * Phase 2 pumps ordered messages; phase 3 consumes the same frame's key table
 * (message_count must then be zero). Cursor flag 1 holds the supplied screen
 * point until changed; omitted keys answer zero for this phase, never held.
 * Flags 2/4 explicitly declare Missions' TextGetMessage/sound non-execution
 * boundaries. Without flag4 sound uses Audio's exact unopened-display domain;
 * initialized audio remains a named dependency.
 * No pointers escape. Descriptor size must equal sizeof(RlDeclaredInput).
 * Keys are unique vkeys 0..255 with signed 32-bit schema answers; polling uses
 * the signed low16. Messages are 0x100/101 or 0x200..205, in exact order.
 * Calls fail atomically for world/clock/generation/cursor/input and preserve
 * old frame events. A stopped attempt publishes its actual input/order prefix
 * through rl_eventos_entrada; this diagnostic is separate from game state.
 * RlPlayerInputEvent is 64 bytes. values by kind:
 * 1 peek: present,msg,wp,lp; 2 get: msg,wp,lp; 3 translate: result;
 * 4 dispatch: msg,wp,lp,hwnd,three LE words of 11 WndProc flag bytes;
 * 5 cursor: x,y; 6 key: vk,signed answer bits;
 * 7 SetPlayerOrdre: player,tag,word2,word4,word6,word8;
 * 8 ProcessInputGame: player,tag,three LE packet words (10 bytes),jump target;
 * 9 TextGetMessage declared boundary: message,EAX;
 * 10 sound boundary: post, EAX,EDX,EBX,ECX,has_volume,volume.
 * phase is 2/3, frame is the original cycle counter. Unused words are padding.
 * Event queries never consume or modify a successful/stopped attempt's log.
 */
typedef struct { uint32_t message,wparam,lparam; } RlInputMessage;
typedef struct { uint32_t vk; int32_t value; } RlInputKey;
typedef struct {
 uint32_t size,flags; int32_t cursor_x,cursor_y;
 const RlInputMessage *messages; uint32_t message_count;
 const RlInputKey *keys; uint32_t key_count;
} RlDeclaredInput;
typedef struct { uint32_t kind,phase,site,frame,values[12]; } RlPlayerInputEvent;
enum { RL_INPUT_CURSOR=1, RL_INPUT_DISPLAY_TEXT_BOUNDARY=2, RL_INPUT_DISPLAY_SOUND_BOUNDARY=4 };
int32_t rl_passo_quadro_entrada(RlMundo world,uint32_t phase,RlFrameInput *input,const RlDeclaredInput *declared);
int32_t rl_eventos_entrada(RlMundo world,RlPlayerInputEvent *buffer,uint32_t capacity,uint32_t *required);
/* ABI1.6: opt-in persistent execution after loading a complete mission.
 * 0 (default): audit execution, complete frame/input/render observations.
 * 1: play execution, no frame/input/render audit records. Order/text/sound
 * events remain available; failed input attempts retain full diagnostics.
 * 2: the same persistent execution with complete audit records for validation.
 * All modes preserve exact game state, RNGs, clock, registers, pixels, cache,
 * named stops and atomic errors. Mode changes do not advance generation/ticks.
 * Sparse fixture loads return to mode0. Complete reloads preserve the mode.
 * Exports remain supported in play mode; only explicitly requested exports
 * perform projection/copies. Select mode1 before the first live frame.
 * Old signatures/layouts/creation versions remain valid. Consumers should
 * accept major1, minor>=their required minor instead of exact version equality.
 */
enum { RL_EXECUCAO_AUDITORIA=0, RL_EXECUCAO_JOGO=1, RL_EXECUCAO_JOGO_AUDITADO=2 };
int32_t rl_execucao_definir(RlMundo world,uint32_t mode);
int32_t rl_execucao_modo(RlMundo world,uint32_t *mode);
/* Rules follow the world: rl_missao_iniciar/_menu take config.regras, the next
 * stage (MissionNext, campaign continuation, rl_shell_campaign_load from a shell
 * created on the menu world) inherits them, and saves record them (schema3 only
 * when regras!=0; a schema2 save loads with regras=0). rl_regras reads them.
 * rl_regras_definir changes them only before the load tail: a menu world, or a
 * stage world that has not run phase1. Afterwards, or with an unknown bit, it
 * returns RL_INVALID_ARGUMENT and the world is unchanged. */
int32_t rl_regras(RlMundo world,uint32_t *regras);
int32_t rl_regras_definir(RlMundo world,uint32_t regras);
/* Pending Shift+click waypoints of one unit, oldest first (cells y*256+x).
 * *count = total; at most capacity cells written (cells may be NULL when
 * capacity==0). Always 0 without RL_REGRA_FILA_ORDENS. Saves record the queue
 * as schema3 section 9, only when it is not empty. */
int32_t rl_fila_ordens(RlMundo world,uint32_t unit,uint16_t *cells,uint32_t capacity,uint32_t *count);
/* P4 save extension v1 to ABI1.6; independent container/state versions.
 * These saves serialize Rust state, not the original GameSave format.
 * At a public-call boundary pass the CURRENT carried RlFrameInput (the exact
 * value to use for the next phase), including saved/msg/GPRs/offscreen.
 * All game memory, RNG, clock/rate/epoch, mode, phase, generation, cursor,
 * transport/audio state, events and accepted FIFO inputs are preserved.
 * Byte snapshots: NULL/0 query returns RL_BUFFER_TOO_SMALL and required size;
 * insufficient capacity writes required only and never a prefix. Save does not
 * mutate the world, clock or queue. Buffers/outputs must be disjoint.
 * Load creates a NEW world handle and copies the saved carried input. Both
 * outputs stay unchanged on error; existing worlds/renderers are untouched.
 * On success switch to the new handle, bind a new renderer using the app's
 * explicit initial surface/cache/assets, and free the old world/renderer.
 * Presentation caches, device playback and unsent host inputs are not in the
 * save. Never bind a cached render result from the previous world to the new one.
 * The saved generation is restored exactly; handles themselves are never saved.
 * Paths are UTF-8 byte spans of 1..32768 bytes, WITHOUT a NUL terminator.
 * File save syncs a unique sibling temp file and renames it over the slot.
 * An error before rename preserves the previous file; a directory-sync error
 * after rename is reported although the replacement has already occurred.
 * Limits: 64MiB per memory image/FIFO, 128MiB uncompressed, 160MiB complete
 * file, 65536 records/queued phases, 256 keys per phase. No pointers retained.
 */
#define RL_SAVE_FORMAT_VERSION 1u
#define RL_SAVE_STATE_SCHEMA 2u
enum { RL_SAVE_INVALID=12, RL_SAVE_CHECKSUM=13, RL_SAVE_VERSION=14,
 RL_IO_ERROR=15, RL_SAVE_BUSY=16, RL_NO_AUTOSAVE=17 };
uint32_t rl_salvar_versao(void);
int32_t rl_salvar_bytes(RlMundo world,const RlFrameInput *input,
 uint8_t *buffer,uint32_t capacity,uint32_t *required);
int32_t rl_carregar_bytes(const uint8_t *bytes,uint32_t length,
 RlMundo *new_world,RlFrameInput *input);
int32_t rl_salvar_arquivo(RlMundo world,const RlFrameInput *input,
 const uint8_t *path,uint32_t path_length);
int32_t rl_carregar_arquivo(const uint8_t *path,uint32_t path_length,
 RlMundo *new_world,RlFrameInput *input);
/* Optional owned FIFO. Same descriptor/phase validation as immediate input.
 * Each queued item is one phase (2 or 3); phase3 cannot have messages. Submit
 * phase2 and its phase3 answers in order. Drain before submitting newer direct
 * input. A pending step updates input and removes one item only on success;
 * an empty FIFO succeeds without mutation. Failed execution retains the item
 * and publishes existing input diagnostics. Loading raw blocks clears the FIFO
 * and autosave; freeing a world frees both. Direct phase APIs remain supported.
 */
int32_t rl_entrada_enfileirar(RlMundo world,uint32_t phase,const RlDeclaredInput *declared);
int32_t rl_entrada_passo_pendente(RlMundo world,RlFrameInput *input);
int32_t rl_entrada_quantidade_pendente(RlMundo world,uint32_t *count);
/* Stage loader hook: after load-tail AND preparing the carried input for play,
 * capture one engine-owned stage-start slot. No disk path, RNG draw or tick.
 * The host invokes the hook for each new stage (including S0 startup); the
 * current engine does not yet own campaign loading. Capture failure preserves
 * the prior slot. The app persists the captured bytes or calls the file export.
 * Reading/persisting does not consume or refresh the slot. A newly loaded world
 * has no autosave slot; disk autosaves remain app-owned. No recursive backups.
 */
int32_t rl_autosalvar_inicio_fase(RlMundo world,const RlFrameInput *input);
int32_t rl_autosalvar_bytes(RlMundo world,uint8_t *buffer,uint32_t capacity,uint32_t *required);
int32_t rl_autosalvar_arquivo(RlMundo world,const uint8_t *path,uint32_t path_length);
#ifdef __cplusplus
}
#endif
#endif
