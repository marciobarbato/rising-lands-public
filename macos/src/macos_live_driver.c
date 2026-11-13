/* App orchestration/transport only. Actual logic and pixels come from Rust ABI1.6. */
#include "MacOSLiveDriver.h"
#include "events_host_file.h"
#include <fcntl.h>
#include <errno.h>
#include <unistd.h>
#include "rising_load_tail_input.h"
#include "rising_engine.h"
#include "rising_difficulty.h"
#include "game_hud_background_abi.h"
#include <CommonCrypto/CommonDigest.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define PRESENTATION_SIZE (480000u+1024u+270336u+4u)
#define VIEW_V3_SIZE (PRESENTATION_SIZE+20u)
#define VIEW_SIZE (VIEW_V3_SIZE+8u)
typedef struct {uint32_t base,length,offset;} Entry;
typedef struct {uint64_t frame;int32_t x,y;MacOSInputMessage message;uint32_t cursor;} QueuedInput;
_Static_assert(sizeof(MacOSDeclaredInput)==sizeof(RlDeclaredInput),"declared ABI size");
_Static_assert(sizeof(MacOSInputMessage)==sizeof(RlInputMessage),"message ABI size");
_Static_assert(sizeof(MacOSInputKey)==sizeof(RlInputKey),"key ABI size");
typedef struct {uint32_t offset,length,zero;} Span;
extern int32_t macos_projection_validate(const uint8_t*,uint32_t,const Entry*,uint32_t);
extern int32_t macos_projection_sha256(const uint8_t*,uint32_t,const Span*,uint32_t,uint8_t*);
#define FN(name) __typeof__(&name) name
/* World rule set for new production worlds (RlMissaoInicio.regras): set by the
 * app before creation from its preferences ("Atalhos modernos" = Shift+click
 * order queue, RL_REGRA_FILA_ORDENS; "Caminhos melhores" = modern unit paths,
 * RL_REGRA_CAMINHOS_MELHORES). Validation tools always use 0. */
static uint32_t default_rules;
struct MacOSLiveDriver {
 void *library;
 RlEventsHostV1 events_host;uint8_t *events_directory,*events_bytes;
 FN(rl_missao_iniciar_eventos_v1);FN(rl_eventos_reassociar_v1);FN(rl_missao_iniciar_menu_eventos_v1);
 FN(rl_shell_menu_init);FN(rl_shell_campaign_reset);FN(rl_shell_resume);
 FN(rl_shell_campaign_load_dificuldade);FN(rl_shell_credits_frame);
 FN(rl_missao_versao);FN(rl_missao_iniciar);FN(rl_missao_nome_proxima);FN(rl_missao_continuar);FN(rl_missao_tela);
 FN(rl_eventos_quadro);FN(rl_aplicar_dificuldade);FN(rl_carregar_eventos);
 FN(rl_versao); FN(rl_criar_mundo); FN(rl_carregar_blocos); FN(rl_carregar_textos); FN(rl_passo_quadro);
 FN(rl_execucao_definir); FN(rl_execucao_modo); FN(rl_load_tail_entrada_v1);FN(rl_passo_quadro_entrada);FN(rl_eventos_entrada);
 FN(rl_liberar_mundo); FN(rl_projetar_tudo); FN(rl_erro_mensagem);
 FN(rl_render_contrato); FN(rl_render_criar); FN(rl_render_geracao); FN(rl_render_quadro); FN(rl_render_liberar);
 FN(rl_render_reset);FN(rl_game_hud_background_version);FN(rl_game_hud_background_initialize);
 FN(rl_render_criar_cd);FN(rl_render_reset_cd);FN(rl_mundo_paleta_jogo);FN(rl_shell_imagem_cd);FN(rl_mundo_tabelas_sombra_luz);
 FN(rl_salvar_versao);FN(rl_salvar_arquivo);FN(rl_salvar_bytes);FN(rl_carregar_arquivo);FN(rl_render_cache);
 FN(rl_autosalvar_inicio_fase);FN(rl_autosalvar_arquivo);FN(rl_autosalvar_bytes);
 FN(rl_entrada_quantidade_pendente);FN(rl_entrada_passo_pendente);
 /* World rules and the Shift+click order queue (optional symbols; absent = original rules only). */
 FN(rl_regras);FN(rl_regras_definir);FN(rl_fila_ordens);
#ifdef MACOS_EVENT_FLC
 FN(rl_eventos_flc_v1);FN(rl_evento_flc_estado_v1);FN(rl_evento_flc_terminar_v1);
#endif
 FN(rl_shell_version);FN(rl_shell_create);FN(rl_shell_plan);FN(rl_shell_enter);FN(rl_shell_paint_prepared);FN(rl_shell_input);FN(rl_shell_draw);FN(rl_shell_copy);FN(rl_shell_free);FN(rl_shell_campaign_prepare);FN(rl_shell_campaign_load);FN(rl_shell_defeat_finish);FN(rl_shell_export_world);
 RlShell shell;uint64_t shell_generation;uint32_t shell_mode;RlShellInput shell_input;RlShellEvent shell_request;
 /* Port-only "Opções" screen (rising_options.h; optional symbols: an engine
  * without them shows no button). One handle for the driver's lifetime. */
 FN(rl_opcoes_versao);FN(rl_opcoes_criar);FN(rl_opcoes_definir);FN(rl_opcoes_entrada);FN(rl_opcoes_sobrepor);FN(rl_opcoes_desenhar);FN(rl_opcoes_botao);FN(rl_opcoes_fechar);FN(rl_opcoes_liberar);FN(rl_opcoes_abrir);
 RlOpcoes options;int options_open;
 char mission[16];uint32_t difficulty;
 /* menu_world: started in the original main menu (no frame state, no renderer).
  * new_campaign: stage START_MISSION loads without a victory: 1 after RESET_CAMPAIGN
  * (Novo jogo), the current stage after the pause menu's "Reiniciar a missão". */
 int menu_world,new_campaign;
 /* Host save captions for the original lists: [present, 32-byte CP1252 caption] x10,
  * slot index0..9 = original save1.sav..save9.sav,save0.sav = modern posicao-1..10. */
 uint8_t slots[10][33];
 /* Gameplay presentation (surface/cache/cursor) retained across the pause menu. */
 uint8_t *pause_presentation,pause_palette[768];
 char *root;char save_error[512];uint8_t *stage_presentation;uint32_t save_capacity;
 int32_t stage_cursor_x,stage_cursor_y;
 int validation,production_input;
 /* Menu and mission worlds: every presentation input comes from the CD files
  * (no prepared palette, tables, terrain cache or shell pictures). */
 int cd_inputs;
 uint8_t *texts;uint32_t text_length;int texts_loaded;
 QueuedInput pending[4096];uint32_t pending_count,input_next_phase;
 MacOSInputMessage frame_messages[4096];MacOSInputKey frame_keys[256];MacOSDeclaredInput frame_input;
 int32_t held[256];int32_t cursor_x,cursor_y;
 uint64_t mouse_until,human_events,declared_frames;
 /* Left button held by the host: a real press stays down until the real release. */
 uint64_t left_down_frame;int32_t left_held,left_x,left_y;
 RlMundo world; RlRenderer renderer; RlFrameInput input,phase2;
 MacOSLiveStatus status;
 uint8_t pixels[480000],palette[768];
 /* Immutable production seed, decoded once per driver. Validation keeps its
  * frozen surface. Inline storage also makes campaign candidates independent. */
 uint8_t game_background[480000];int game_background_ready,game_surface;
 uint8_t *bulk,*surface_hashes,*state_hashes; uint32_t bulk_capacity,surface_count,state_count,audit_limit;
 /* "Atalhos modernos" read-only snapshot buffer (macos_live_peek), play only. */
 uint8_t *peek; uint32_t peek_capacity;
 Entry *entries; Span *spans; uint32_t entry_count,span_count;
 /* Play-mode sound request transport (kind10 copies plus Manage ticks). */
 int sound_capture; RlPlayerInputEvent *events; uint32_t event_capacity,sound_count; uint64_t sound_dropped,sound_errors;
 MacOSSoundEvent sounds[4096];
};
uint32_t macos_live_abi_version(void){return 1;}
uint32_t macos_live_engine_binding(void){
#ifdef MACOS_STATIC_ENGINE
 return 1;
#else
 return 0;
#endif
}
uint32_t macos_live_status_size(void){return sizeof(MacOSLiveStatus);}
static uint64_t now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000+t.tv_nsec;}
static uint32_t le(const uint8_t*p){return p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static uint8_t *file(const char *root,const char *name,uint32_t *length){
 char path[4096];snprintf(path,sizeof(path),"%s/%s",root,name);FILE*f=fopen(path,"rb");if(!f)return NULL;
 if(fseek(f,0,SEEK_END)){fclose(f);return NULL;}long n=ftell(f);rewind(f);
 if(n<=0||n>128*1024*1024){fclose(f);return NULL;}uint8_t*p=malloc((size_t)n);if(!p){fclose(f);return NULL;}
 if(fread(p,1,(size_t)n,f)!=(size_t)n){free(p);p=NULL;}fclose(f);if(p)*length=(uint32_t)n;return p;
}
static int fail(MacOSLiveDriver*d,int code,const char*context){
 d->status.stopped=1;uint32_t n=0;char detail[320]={0};
 d->rl_erro_mensagem(NULL,0,&n);if(n&&n<=sizeof(detail))d->rl_erro_mensagem((uint8_t*)detail,n,&n);
 snprintf(d->status.message,sizeof(d->status.message),"%s · quadro %llu · ABI %d: %s",context,(unsigned long long)(d->status.frames+1),code,detail);return code?code:5;
}
/* Startup/next-stage transport only. MissionNext inherits the owned resource;
 * older saved worlds may not have it. Never reinstall into a loaded save. */
static int event_flc_declare(MacOSLiveDriver*d,RlMundo world);
static int load_events_host(MacOSLiveDriver*d,RlMundo world,int inherited);
/* EVENTS host transport, then the FLC event table of the same world. */
static int load_events(MacOSLiveDriver*d,RlMundo world,int inherited){
 int c=load_events_host(d,world,inherited);
 return c?c:event_flc_declare(d,world);
}
static int load_events_host(MacOSLiveDriver*d,RlMundo world,int inherited){
 if(d->events_host.version)return d->rl_eventos_reassociar_v1(world,&d->events_host);
 if(inherited){
  uint32_t n=0;int c=d->rl_projetar_tudo(world,NULL,0,&n);if(c!=RL_BUFFER_TOO_SMALL)return c?c:RL_SAVE_INVALID;
  uint8_t*data=malloc(n);if(!data)return RL_IO_ERROR;c=d->rl_projetar_tudo(world,data,n,&n);
  uint32_t off=4;int found=0;
  if(!c&&n<4)c=RL_SAVE_INVALID;
  if(!c)for(uint32_t i=0;i<le(data);i++){
   if((uint64_t)off+8>n){c=RL_SAVE_INVALID;break;}uint32_t base=le(data+off),length=le(data+off+4);off+=8;
   if((uint64_t)off+length>n){c=RL_SAVE_INVALID;break;}if(base==0x06200000)found=1;off+=length;
  }
  free(data);if(c||found)return c;
 }
 uint32_t n=0;uint8_t*bytes=file(d->root,"EVENTS.INI",&n);
 if(!bytes){snprintf(d->save_error,sizeof(d->save_error),"EVENTS.INI ausente nos dados nativos; recompile o aplicativo");return RL_IO_ERROR;}
 int c=d->rl_carregar_eventos(world,bytes,n);free(bytes);return c;
}
static int proof(MacOSLiveDriver*d,uint32_t index,uint32_t phase,MacOSPhaseObserver observer,void*ctx){
 d->status.phase_index=index;d->status.stop_phase=phase;
 if(!d->validation)return 0;
 uint64_t t=now();
 if(index<d->audit_limit){
  uint32_t n=0;int c=d->rl_projetar_tudo(d->world,d->bulk,d->bulk_capacity,&n);
  if(c)return fail(d,c,"projeção de prova");
  if(macos_projection_validate(d->bulk,n,d->entries,d->entry_count)||macos_projection_sha256(d->bulk,n,d->spans,d->span_count,d->status.state_sha)||memcmp(d->status.state_sha,d->state_hashes+32*index,32)){
   d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"StateProofMismatch na fase %u",index);return 5;
  }
  d->status.phases_equal++;
  const uint32_t checkpoints[]={1,31,32,45,50,51,52,64,100,101,128};
  if(phase==3)for(unsigned i=0;i<11;i++)if((index/2)==checkpoints[i])d->status.checkpoints_equal++;
 }
 if(observer&&observer(ctx,d->world,index,phase)){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"PhaseObserverRejected %u",index);return 5;}
 d->status.proof_ns+=now()-t;return 0;
}
/* Renderer inputs are explicit original assets/cache, never a cache bound to
 * the pre-load world. Candidate construction cannot alter the current driver. */
static int game_background(MacOSLiveDriver*d,const uint8_t*pie,uint32_t length){
 if(d->game_background_ready)return 0;
 uint8_t*owned=NULL;if(!pie){owned=file(d->root,"asset6.bin",&length);pie=owned;}
 int c=pie?d->rl_game_hud_background_initialize(RL_GAME_HUD_BACKGROUND_VERSION,pie,length,d->game_background,sizeof(d->game_background)):RL_IO_ERROR;
 free(owned);if(!c)d->game_background_ready=1;return c;
}
/* Reconstruct retained production rendering through the existing reset API:
 * saved tooltip bytes, cached outcome and display/cache state are invalidated.
 * Never called by advance or by scale-only window resizing. */
static int reset_game_surface(MacOSLiveDriver*d){
 if(d->game_surface)return 0;
 int c=game_background(d,NULL,0);if(c)return c;
 if(d->cd_inputs){RlRenderBytes surface={d->game_background,480000};c=d->rl_render_reset_cd(d->renderer,&surface);if(!c){memcpy(d->pixels,d->game_background,480000);d->game_surface=1;}return c;}
 uint32_t n=0,cn=0;uint8_t*keys=file(d->root,"keys.bin",&n);RlRenderBytes kd={keys,n};
 uint8_t*raw=file(d->root,"cache.bin",&n);RlRenderBytes rd={raw,n};uint8_t*cursor=file(d->root,"cursor.bin",&cn);
 RlRenderBytes surface={d->game_background,480000};RlRenderCache cache={kd,rd,cursor&&cn==4?le(cursor):UINT32_MAX};
 c=keys&&raw&&cursor?d->rl_render_reset(d->renderer,&surface,&cache):RL_IO_ERROR;
 free(keys);free(raw);free(cursor);
 if(!c){memcpy(d->pixels,d->game_background,480000);d->game_surface=1;}return c;
}
static int new_renderer_cd(MacOSLiveDriver*d,RlMundo world,const uint8_t*presentation,RlRenderer*out){
 RlRenderBytes assets[7]={0};uint8_t*back[7]={0};char name[40];uint32_t n=0;int c=1;
 for(unsigned i=0;i<7;i++){snprintf(name,sizeof(name),"asset%u.bin",i);back[i]=file(d->root,name,&n);assets[i]=(RlRenderBytes){back[i],n};if(!back[i])goto done;}
 if(presentation){
  /* A saved presentation carries its own surface and terrain cache. */
  RlRenderBytes sd={presentation,480000};
  RlRenderCache cache={{presentation+480000,1024},{presentation+481024,270336},le(presentation+751360)};
  c=d->rl_render_criar_cd(world,assets,7,&sd,out);
  if(!c){c=d->rl_render_reset(*out,&sd,&cache);if(c)d->rl_render_liberar(*out);}
 }else{
  c=game_background(d,back[6],assets[6].length);
  if(!c){RlRenderBytes sd={d->game_background,480000};c=d->rl_render_criar_cd(world,assets,7,&sd,out);}
  if(!c){memcpy(d->pixels,d->game_background,480000);d->game_surface=1;}
 }
 done:for(unsigned i=0;i<7;i++)free(back[i]);return c;
}
static int new_renderer(MacOSLiveDriver*d,RlMundo world,const uint8_t*presentation,RlRenderer*out){
 if(d->cd_inputs)return new_renderer_cd(d,world,presentation,out);
 RlRenderBytes assets[9]={0};uint8_t*back[9]={0};char name[40];uint32_t n=0;int c=1;
 for(unsigned i=0;i<9;i++){snprintf(name,sizeof(name),"asset%u.bin",i);back[i]=file(d->root,name,&n);assets[i]=(RlRenderBytes){back[i],n};if(!back[i])goto done;}
 uint8_t*surface=file(d->root,"surface.bin",&n);RlRenderBytes sd={surface,n};
 uint8_t*keys=file(d->root,"keys.bin",&n);RlRenderBytes kd={keys,n};
 uint8_t*payloads=file(d->root,"cache.bin",&n);RlRenderBytes pd={payloads,n};
 uint32_t cursor_size=0;uint8_t*cursor=file(d->root,"cursor.bin",&cursor_size);
 RlRenderCache cache={kd,pd,cursor&&cursor_size==4?le(cursor):UINT32_MAX};
 if(presentation){sd=(RlRenderBytes){presentation,480000};cache=(RlRenderCache){{presentation+480000,1024},{presentation+481024,270336},le(presentation+751360)};}
 else if(!d->validation){c=game_background(d,back[6],assets[6].length);if(c)goto buffers;sd=(RlRenderBytes){d->game_background,480000};}
 if(sd.data&&keys&&payloads&&cursor)c=d->rl_render_criar(world,assets,9,&sd,&cache,out);else c=RL_IO_ERROR;
 if(!c&&!presentation){memcpy(d->pixels,sd.data,480000);d->game_surface=!d->validation;}
 buffers:
 free(surface);free(keys);free(payloads);free(cursor);
 done:for(unsigned i=0;i<9;i++)free(back[i]);return c;
}
static int capture_presentation(MacOSLiveDriver*d,uint8_t*out){
 uint32_t keys=0,payloads=0,cursor=0;
 int c=d->rl_render_cache(d->renderer,out+480000,1024,out+481024,270336,&keys,&payloads,&cursor);
 if(c)return c;if(keys!=1024||payloads!=270336)return 5;
 memcpy(out,d->pixels,480000);memcpy(out+751360,&cursor,4);return 0;
}
/* Publish a complete initial play image after the real load tail reaches mode18.
 * This is drawing only: no extra game phase/tick/input/RNG call. The next frame's
 * same-generation render request reuses the existing renderer outcome cache. */
static int first_game_draw(MacOSLiveDriver*d){
 uint64_t started=now();
 uint64_t generation=0;uint32_t phase=0,frame=0;RlRenderResult out={0};
 int c=d->rl_render_geracao(d->world,&generation,&phase,&frame);
 if(!c)c=d->rl_render_quadro(d->renderer,generation,1,d->pixels,sizeof(d->pixels),d->palette,sizeof(d->palette),&out);
 d->status.render_ns+=now()-started;
 if(c)return fail(d,c,"primeiro desenho do jogo");
 if(out.abi_status||out.generation!=generation||out.width!=800||out.height!=600||out.stride!=800||out.required_pixels!=480000||out.required_palette!=768||out.palette_generation!=1||out.disposition>2)return fail(d,RL_UNSUPPORTED_PATH,"contrato do primeiro desenho");
 if(out.disposition==2){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"%s at 0x%08x: %s",out.stop_callee,out.stop_address,out.stop_reason);return RL_UNSUPPORTED_PATH;}
 d->status.generation=generation;d->status.disposition=out.disposition;return 0;
}
#ifdef MACOS_STATIC_ENGINE
#define LOAD(name) do {d->name=&name;} while(0)
#define LOAD_OPTIONAL(name) do {d->name=&name;} while(0)
#else
#define LOAD(name) do { d->name=(__typeof__(d->name))dlsym(d->library,#name);if(!d->name){snprintf(error,capacity,"Missing ABI symbol %s",#name);goto bad;} } while(0)
#define LOAD_OPTIONAL(name) do { d->name=(__typeof__(d->name))dlsym(d->library,#name); } while(0)
#endif
static int shell_open(MacOSLiveDriver*d,uint32_t mode);
static MacOSLiveDriver *create(const char*library,const char*root,const char*mission,uint32_t difficulty,const RlEventsHostV1*host,char*error,uint32_t capacity,int menu){
 MacOSLiveDriver*d=calloc(1,sizeof(*d));if(!d)return NULL;d->status.phase_index=UINT32_MAX;d->validation=1;
 snprintf(d->mission,sizeof(d->mission),"%s",mission?mission:menu?"MENU":"MONDE01.ETA");d->difficulty=difficulty;
if(host){
 if(host->size!=sizeof(*host)||host->version!=1||!host->directory||!host->events||host->directory_length<4||host->directory_length>244||!host->events_length||host->events_length>1024*1024){snprintf(error,capacity,"Contrato explícito de EVENTS inválido");goto bad;}
 d->events_directory=malloc(host->directory_length);d->events_bytes=malloc(host->events_length);if(!d->events_directory||!d->events_bytes)goto bad;
 memcpy(d->events_directory,host->directory,host->directory_length);memcpy(d->events_bytes,host->events,host->events_length);
 d->events_host=*host;d->events_host.directory=d->events_directory;d->events_host.events=d->events_bytes;
}
 d->root=strdup(root);if(!d->root)goto bad;
#ifndef MACOS_STATIC_ENGINE
 d->library=dlopen(library,RTLD_NOW|RTLD_LOCAL);if(!d->library){snprintf(error,capacity,"%s",dlerror());goto bad;}
#endif
 if(host){LOAD(rl_missao_iniciar_eventos_v1);LOAD(rl_eventos_reassociar_v1);}
#ifdef MACOS_EVENT_FLC
 LOAD(rl_eventos_flc_v1);LOAD(rl_evento_flc_estado_v1);LOAD(rl_evento_flc_terminar_v1);
#endif
 if(menu){if(!host){snprintf(error,capacity,"O menu original exige o mapeamento de EVENTS");goto bad;}LOAD(rl_missao_iniciar_menu_eventos_v1);}
 LOAD(rl_shell_menu_init);LOAD(rl_shell_campaign_reset);LOAD(rl_shell_resume);
 LOAD(rl_shell_campaign_load_dificuldade);LOAD(rl_shell_credits_frame);
 LOAD(rl_missao_versao);LOAD(rl_missao_iniciar);LOAD(rl_missao_nome_proxima);LOAD(rl_missao_continuar);LOAD(rl_missao_tela);
 LOAD(rl_eventos_quadro);LOAD(rl_aplicar_dificuldade);LOAD(rl_carregar_eventos);
 LOAD(rl_execucao_definir);LOAD(rl_execucao_modo);LOAD(rl_versao);LOAD(rl_criar_mundo);LOAD(rl_carregar_blocos);LOAD(rl_carregar_textos);LOAD(rl_passo_quadro);LOAD(rl_liberar_mundo);LOAD(rl_load_tail_entrada_v1);LOAD(rl_passo_quadro_entrada);LOAD(rl_eventos_entrada);LOAD(rl_projetar_tudo);LOAD(rl_erro_mensagem);
 LOAD(rl_render_contrato);LOAD(rl_render_criar);LOAD(rl_render_geracao);LOAD(rl_render_quadro);LOAD(rl_render_liberar);
 LOAD(rl_render_reset);LOAD(rl_game_hud_background_version);LOAD(rl_game_hud_background_initialize);
 LOAD(rl_render_criar_cd);LOAD(rl_render_reset_cd);LOAD(rl_mundo_paleta_jogo);LOAD(rl_shell_imagem_cd);LOAD(rl_mundo_tabelas_sombra_luz);
 LOAD(rl_salvar_versao);LOAD(rl_salvar_arquivo);LOAD(rl_salvar_bytes);LOAD(rl_carregar_arquivo);LOAD(rl_render_cache);LOAD(rl_autosalvar_inicio_fase);LOAD(rl_autosalvar_arquivo);LOAD(rl_autosalvar_bytes);
 LOAD(rl_entrada_quantidade_pendente);LOAD(rl_entrada_passo_pendente);
 LOAD_OPTIONAL(rl_regras);LOAD_OPTIONAL(rl_regras_definir);LOAD_OPTIONAL(rl_fila_ordens);
 LOAD_OPTIONAL(rl_opcoes_versao);LOAD_OPTIONAL(rl_opcoes_criar);LOAD_OPTIONAL(rl_opcoes_definir);LOAD_OPTIONAL(rl_opcoes_entrada);LOAD_OPTIONAL(rl_opcoes_sobrepor);LOAD_OPTIONAL(rl_opcoes_desenhar);LOAD_OPTIONAL(rl_opcoes_botao);LOAD_OPTIONAL(rl_opcoes_fechar);LOAD_OPTIONAL(rl_opcoes_liberar);LOAD_OPTIONAL(rl_opcoes_abrir);
 LOAD(rl_shell_version);LOAD(rl_shell_create);LOAD(rl_shell_plan);LOAD(rl_shell_enter);LOAD(rl_shell_paint_prepared);LOAD(rl_shell_input);LOAD(rl_shell_draw);LOAD(rl_shell_copy);LOAD(rl_shell_free);LOAD(rl_shell_campaign_prepare);LOAD(rl_shell_campaign_load);LOAD(rl_shell_defeat_finish);LOAD(rl_shell_export_world);
 if(d->rl_shell_version()!=RL_SHELL_VERSION){snprintf(error,capacity,"Prepared shell v1 required");goto bad;}
 if(d->rl_salvar_versao()!=RL_SAVE_FORMAT_VERSION){snprintf(error,capacity,"Save extension v1 required");goto bad;}
 if(d->rl_game_hud_background_version()!=RL_GAME_HUD_BACKGROUND_VERSION){snprintf(error,capacity,"Gameplay background extension v1 required");goto bad;}
 if((d->rl_versao()>>16)!=1||(d->rl_versao()&0xffff)<6||d->rl_render_contrato()!=0x10009){snprintf(error,capacity,"ABI1.6 / Graphics1.9 required");goto bad;}
 uint32_t n=0;int c=0;uint8_t*in=NULL;
 /* The ABI's host input context (no game data): the original WinMain entry
  * registers before load_tail (phase 1) and between frames (phase 2), the
  * offscreen surface binding and the message slot. Formerly input1/input2.bin. */
 d->input=(RlFrameInput){0,0,0x60076000u,{1,0,0,0,0,0,0,0x2007ec0u},{0,0,0},{0x3000000u,0,0,0}};
 d->phase2=(RlFrameInput){0,0,0x60076000u,{0,3,0,0x2007fe8u,0,0,0,0x2007fe8u},{0,0,0},{0x3000000u,0,0,0}};
 d->cursor_x=d->input.cursor_x;d->cursor_y=d->input.cursor_y;d->cd_inputs=mission||menu;
 if(mission){
  if(d->rl_missao_versao()!=1){snprintf(error,capacity,"Mission extension v1 required");goto bad;}
  const char*names[]={"RISING.EXE","RISING.INI","RISING.RUL","RISING.SBL","TABLEAU.COL","COULEUR.MAP","PLATGAME.PIE","MONSTRE.INI","TEXTES.TXT",mission};
  RlBloco inputs[10]={0};uint8_t*data[10]={0};
  for(unsigned i=0;i<10;i++){data[i]=file(root,names[i],&n);inputs[i]=(RlBloco){0,n,data[i]};if(!data[i]){snprintf(error,capacity,"Dados da missão ausentes: %s",names[i]);c=1;break;}}
  RlMissaoInicio config={0};config.version=1;config.allocation_fill=0;config.ai_clock_ms=0x123456;
  config.thread_id=1;config.ticks_per_second=20;config.execution_mode=RL_EXECUCAO_JOGO;
  config.iterations[0]=5;config.iterations[1]=2;config.iterations[2]=1;
  config.input=d->input;config.input.cursor_x=400;config.input.cursor_y=300;
  config.regras=d->rl_regras_definir?default_rules:0;
  if(!c&&host)c=d->rl_missao_iniciar_eventos_v1(inputs,10,(const uint8_t*)mission,(uint32_t)strlen(mission),&config,&d->events_host,&d->world,&d->input);
  if(!c&&!host)c=d->rl_missao_iniciar(inputs,10,(const uint8_t*)mission,(uint32_t)strlen(mission),&config,&d->world,&d->input);
  /* Keep the original menu cursor through load_tail. The separately retained
   * host cursor is first sampled by GetCursorPos in between_frames1; moving
   * that sample before load_tail changes scrolling and permutation consumption. */
  for(unsigned i=0;i<10;i++)free(data[i]);
  if(c){if(!error[0]){fail(d,c,"MissionLoad 0x46c2cf");snprintf(error,capacity,"%s",d->status.message);}goto bad;}
  c=d->rl_aplicar_dificuldade(d->world,difficulty);
  if(c){fail(d,c,"Dificuldade");snprintf(error,capacity,"%s",d->status.message);goto bad;}
  c=load_events(d,d->world,0);
  if(c){if(d->save_error[0])snprintf(error,capacity,"%s",d->save_error);else{fail(d,c,"EVENTS.INI");snprintf(error,capacity,"%s",d->status.message);}goto bad;}
  d->validation=0;d->production_input=1;d->texts_loaded=1;
 }else if(menu){
  /* Original program start into MenuInit(local player,2): the first nine
   * stage descriptors (no ETA). The world has no frame state and no renderer;
   * its execution mode is set on the stage world after campaign load. */
  if(d->rl_missao_versao()!=1){snprintf(error,capacity,"Mission extension v1 required");goto bad;}
  const char*names[]={"RISING.EXE","RISING.INI","RISING.RUL","RISING.SBL","TABLEAU.COL","COULEUR.MAP","PLATGAME.PIE","MONSTRE.INI","TEXTES.TXT"};
  RlBloco inputs[9]={0};uint8_t*data[9]={0};
  for(unsigned i=0;i<9;i++){data[i]=file(root,names[i],&n);inputs[i]=(RlBloco){0,n,data[i]};if(!data[i]){snprintf(error,capacity,"Dados do menu ausentes: %s",names[i]);c=1;break;}}
  RlMissaoInicio config={0};config.version=1;config.allocation_fill=0;config.ai_clock_ms=0x123456;
  config.thread_id=1;config.ticks_per_second=20;config.execution_mode=RL_EXECUCAO_JOGO;
  config.iterations[0]=5;config.iterations[1]=2;config.iterations[2]=1;
  config.input=d->input;config.input.cursor_x=400;config.input.cursor_y=300;
  config.regras=d->rl_regras_definir?default_rules:0;
  if(!c)c=d->rl_missao_iniciar_menu_eventos_v1(inputs,9,&config,&d->events_host,&d->world,&d->input);
  for(unsigned i=0;i<9;i++)free(data[i]);
  if(c){if(!error[0]){fail(d,c,"MenuInit 0x4682bb");snprintf(error,capacity,"%s",d->status.message);}goto bad;}
  d->cursor_x=d->input.cursor_x;d->cursor_y=d->input.cursor_y;
  d->validation=0;d->production_input=1;d->texts_loaded=1;d->menu_world=1;
 }else{
uint8_t*s0=file(root,"s0.bin",&n);if(!s0){snprintf(error,capacity,"Missing S0");goto bad;}
 d->bulk_capacity=n;d->bulk=malloc(n);uint32_t count=n>=4?le(s0):0,off=4;if(!count||count>65536){free(s0);goto bad;}RlBloco*blocks=calloc(count,sizeof(*blocks));
 if(!d->bulk||!blocks||count>65536){free(s0);free(blocks);goto bad;}
 for(uint32_t i=0;i<count;i++){if((uint64_t)off+8>n){free(s0);free(blocks);goto bad;}blocks[i].address=le(s0+off);blocks[i].length=le(s0+off+4);off+=8;if((uint64_t)off+blocks[i].length>n){free(s0);free(blocks);goto bad;}blocks[i].data=s0+off;off+=blocks[i].length;}
 c=d->rl_criar_mundo(0x10006,20,0,&d->world);if(!c)c=d->rl_carregar_blocos(d->world,blocks,count);free(blocks);free(s0);if(c){fail(d,c,"S0");snprintf(error,capacity,"%s",d->status.message);goto bad;}
 }
 /* asset0..5 are the CD's six CMP banks and asset6 its PLATGAME.PIE; the
  * prepared tables 7/8 and cache are read only on the historical S0 path. */
 unsigned asset_count=d->cd_inputs?7:9;
 RlRenderBytes assets[9]={0};uint8_t*back[9]={0};char name[40];
 for(unsigned i=0;i<asset_count;i++){snprintf(name,sizeof(name),"asset%u.bin",i);back[i]=file(root,name,&n);assets[i]=(RlRenderBytes){back[i],n};if(!back[i]){for(unsigned j=0;j<=i;j++)free(back[j]);goto bad;}}
 uint8_t*surface=NULL,*keys=NULL,*payloads=NULL,*cursor=NULL;RlRenderBytes sd={0};RlRenderCache cache={0};
 if(!d->cd_inputs){
  surface=file(root,"surface.bin",&n);sd=(RlRenderBytes){surface,n};keys=file(root,"keys.bin",&n);RlRenderBytes kd={keys,n};payloads=file(root,"cache.bin",&n);RlRenderBytes pd={payloads,n};
  uint32_t cursor_size=0;cursor=file(root,"cursor.bin",&cursor_size);cache=(RlRenderCache){kd,pd,cursor&&cursor_size==4?le(cursor):UINT32_MAX};
 }
 c=0;
 if(!d->validation){c=game_background(d,back[6],assets[6].length);if(!c)sd=(RlRenderBytes){d->game_background,480000};}
 if(!c&&!menu&&d->cd_inputs)c=sd.data?d->rl_render_criar_cd(d->world,assets,7,&sd,&d->renderer):1;
 else if(!c&&!menu){if(sd.data&&keys&&payloads&&cursor)c=d->rl_render_criar(d->world,assets,9,&sd,&cache,&d->renderer);else c=1;}
 if(!c&&!menu&&sd.length==480000){memcpy(d->pixels,sd.data,480000);d->game_surface=!d->validation;}
 free(surface);free(keys);free(payloads);free(cursor);for(unsigned i=0;i<9;i++)free(back[i]);
 if(c){fail(d,c,"renderer create");snprintf(error,capacity,"%s",d->status.message);goto bad;}
 if(!mission&&!menu){
 d->entries=(Entry*)file(root,"entries.bin",&n);d->entry_count=n/sizeof(Entry);if(!d->entries||n%sizeof(Entry))goto bad;
 d->spans=(Span*)file(root,"spans.bin",&n);d->span_count=n/sizeof(Span);if(!d->spans||n%sizeof(Span))goto bad;
 d->state_hashes=file(root,"states.bin",&n);if(!d->state_hashes||n%32||n<257*32)goto bad;d->state_count=n/32;d->audit_limit=257;
 d->surface_hashes=file(root,"surfaces.bin",&n);if(!d->surface_hashes||n%32||n<128*32)goto bad;d->surface_count=n/32;
 }
 if(d->cd_inputs){c=d->rl_mundo_paleta_jogo(d->world,d->palette,768);if(c){fail(d,c,"paleta do jogo");snprintf(error,capacity,"%s",d->status.message);goto bad;}}
 else{in=file(root,"palette.bin",&n);if(!in||n!=768){free(in);goto bad;}memcpy(d->palette,in,768);free(in);}
 d->texts=file(root,"TEXTES.TXT",&d->text_length);
 if(menu){c=shell_open(d,RL_SHELL_MAIN);if(c){snprintf(error,capacity,"%s",d->save_error[0]?d->save_error:"Menu original indisponível");goto bad;}}
 return d;
 bad:if(capacity&&error[0]==0)snprintf(error,capacity,"Invalid native initialization inputs");macos_live_free(d);return NULL;
}
MacOSLiveDriver *macos_live_create(const char*library,const char*root,char*error,uint32_t capacity){return create(library,root,NULL,1,NULL,error,capacity,0);}
MacOSLiveDriver *macos_live_create_mission(const char*library,const char*root,const char*name,uint32_t difficulty,char*error,uint32_t capacity){
 if(!name||strchr(name,'/')||strchr(name,92)){if(error&&capacity)snprintf(error,capacity,"Nome de missão inválido");return NULL;}
 return create(library,root,name,difficulty,NULL,error,capacity,0);
}
MacOSLiveDriver *macos_live_create_mission_eventos_v1(const char*library,const char*root,const char*name,uint32_t difficulty,const RlEventsHostV1*host,char*error,uint32_t capacity){
 if(!host||!name||strchr(name,'/')||strchr(name,92)){if(error&&capacity)snprintf(error,capacity,"Contrato explícito de missão/EVENTS inválido");return NULL;}
 return create(library,root,name,difficulty,host,error,capacity,0);
}
int32_t macos_live_validation(MacOSLiveDriver*d,uint32_t enabled){
 if(!d||d->status.phase_index!=UINT32_MAX||enabled>2||d->status.stopped||(enabled&&d->texts_loaded))return 1;
 if(d->menu_world)return 0; /* Already production; the stage world gets its mode after load. */
 if(!enabled){int c=reset_game_surface(d);if(c)return fail(d,c,"fundo original do jogo");}
 if(!enabled&&!d->texts_loaded){
  if(!d->texts){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"TEXTES.TXT ausente nos dados nativos; recompile o aplicativo");return 1;}
  int c=d->rl_carregar_textos(d->world,d->texts,d->text_length,0);
  if(c)return fail(d,c,"InitTextLangue 0x479383");
  c=load_events(d,d->world,0);
  if(c){if(d->save_error[0]){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"%s",d->save_error);return c;}return fail(d,c,"EVENTS.INI");}
  d->texts_loaded=1;
 }
 int c=d->rl_execucao_definir(d->world,enabled==1?RL_EXECUCAO_AUDITORIA:enabled==2?RL_EXECUCAO_JOGO_AUDITADO:RL_EXECUCAO_JOGO);
 if(c)return fail(d,c,"execution mode");
 d->validation=enabled==1;d->production_input=enabled==0;return 0;
}
static void sound_push(MacOSLiveDriver*d,MacOSSoundEvent e){if(d->sound_count==4096){d->sound_dropped++;return;}d->sounds[d->sound_count++]=e;}
/* Copies the engine's ordered kind-10 requests after each play phase
 * (values[0]: 1 PostSound, 0 StopSound, 2 StopAllSound -> mapper kind 3). The WinMain
 * loop calls PlaySnd_Manage before every PeekMessage: one tick opens phase 2 and,
 * when phase 2 dispatched requests, the loop's final tick follows them. Queries
 * never consume or modify engine state; failures only count, never stop play. */
static void sound_collect(MacOSLiveDriver*d,uint32_t phase){
 uint32_t n=0;int c=d->rl_eventos_entrada(d->world,d->events,d->event_capacity,&n);
 if(c==6&&n>d->event_capacity&&n<=65536){RlPlayerInputEvent*grown=realloc(d->events,(size_t)n*sizeof(*grown));if(grown){d->events=grown;d->event_capacity=n;c=d->rl_eventos_entrada(d->world,d->events,d->event_capacity,&n);}}
 if(c){d->sound_errors++;return;}
 uint32_t frame=(uint32_t)(d->status.frames+1),requests=0;
 if(phase==2)sound_push(d,(MacOSSoundEvent){0,frame,2,0x460220,0,0,0,0,0,0});
 for(uint32_t i=0;i<n;i++){RlPlayerInputEvent*e=&d->events[i];if(e->kind!=10)continue;
  sound_push(d,(MacOSSoundEvent){e->values[0]==2?3:e->values[0]?1:2,e->frame,e->phase,e->site,e->values[1],e->values[2],e->values[3],e->values[4],e->values[6],e->values[5]});requests++;}
 if(phase==2&&requests)sound_push(d,(MacOSSoundEvent){0,frame,2,0x460220,0,0,0,0,0,0});
}
int32_t macos_live_sound_capture(MacOSLiveDriver*d,uint32_t enabled){if(!d||enabled>1)return 1;d->sound_capture=(int)enabled;d->sound_count=0;return 0;}
uint32_t macos_live_sound_events(MacOSLiveDriver*d,MacOSSoundEvent*out,uint32_t capacity){
 if(!d||!out)return 0;uint32_t n=d->sound_count<capacity?d->sound_count:capacity;
 memcpy(out,d->sounds,n*sizeof(*out));memmove(d->sounds,d->sounds+n,(d->sound_count-n)*sizeof(*out));d->sound_count-=n;return n;
}
uint64_t macos_live_sound_dropped(MacOSLiveDriver*d){return d?d->sound_dropped+d->sound_errors:0;}
static int input_error(MacOSLiveDriver*d,const char*name){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"%s",name);return 1;}
int32_t macos_live_event(MacOSLiveDriver*d,uint32_t kind,uint32_t msg,int32_t x,int32_t y,uint32_t wp,uint32_t vk,uint32_t down){
 if(!d||d->validation||d->status.stopped)return 1;
 uint64_t frame=d->status.frames+1;if(!d->production_input&&frame<129)frame=129;
 if(kind==1){
  if(msg<0x200||msg>0x205||x<0||x>=800||y<0||y>=600)return input_error(d,"InvalidMacMouseEvent");
  if(msg==0x205||(msg==0x202&&!d->left_held)){d->human_events++;return 0;}
  /* Left press/release follow the host: a press is held across frames (the
   * original MK_LBUTTON drag state) until the actual release, which is never
   * earlier than the fixed k/k+1/k+3 click gesture. A quick click therefore
   * produces the same messages and frames as before. Right clicks keep the
   * fixed gesture and ignore the host release. */
  int left=msg==0x201||msg==0x203,release=msg==0x202;
  uint32_t count=msg==0x200||release?1:left?2:3;
  /* A press without a delivered host release (lost event) closes the old press first. */
  if(left&&d->left_held){int32_t c=macos_live_event(d,1,0x202,d->left_x,d->left_y,0,0,0);if(c)return c;d->human_events--;frame=d->status.frames+1;if(!d->production_input&&frame<129)frame=129;}
  if(d->pending_count+count>4096)return input_error(d,"MacInputQueueCapacityExceeded");
  if(frame<=d->mouse_until)frame=d->mouse_until+1;
  uint32_t lp=((uint32_t)y<<16)|(uint32_t)x,button=d->left_held?1u:0u;
  if(release){
   if(frame<d->left_down_frame+2)frame=d->left_down_frame+2;
   d->pending[d->pending_count++]=(QueuedInput){frame,x,y,{0x202,wp&~3u,lp},1};
   d->mouse_until=frame;d->left_held=0;d->left_x=x;d->left_y=y;
  }else{
   d->pending[d->pending_count++]=(QueuedInput){frame,x,y,{0x200,(wp&~3u)|button,lp},1};
   if(left){
    d->pending[d->pending_count++]=(QueuedInput){frame+1,x,y,{msg,(wp&~3u)|1u,lp},0};
    d->mouse_until=frame+1;d->left_down_frame=frame+1;d->left_held=1;d->left_x=x;d->left_y=y;
   }else if(count==3){
    d->pending[d->pending_count++]=(QueuedInput){frame+1,x,y,{msg,(wp&~3u)|2u|button,lp},0};
    d->pending[d->pending_count++]=(QueuedInput){frame+3,x,y,{0x205,(wp&~3u)|button,lp},0};d->mouse_until=frame+3;
   }else if(button){d->left_x=x;d->left_y=y;}
  }
 }else if(kind==2){
  /* Tab (9) is WndProc-only like the digits: its key-up toggles the original minimap. */
  const uint32_t keys[]={13,16,17,18,27,32,37,38,39,40,46,88,48,49,50,51,52,53,54,55,56,57,9},scans[]={0x1c,0x2a,0x1d,0x38,1,0x39,0x4b,0x48,0x4d,0x50,0x53,0x2d,0x0b,2,3,4,5,6,7,8,9,10,0x0f};
  unsigned i=0;for(;i<23&&keys[i]!=vk;i++){}if(i==23||down>1)return input_error(d,"UnsupportedMacVirtualKey");
  if(d->pending_count==4096)return input_error(d,"MacInputQueueCapacityExceeded");
  uint32_t lp=1|(scans[i]<<16)|((uint32_t)(vk==37||vk==38||vk==39||vk==40||vk==46)<<24)|((uint32_t)(d->held[18]!=0)<<29)|((uint32_t)(d->held[vk]!=0)<<30)|((uint32_t)!down<<31);
  d->pending[d->pending_count++]=(QueuedInput){frame,0,0,{down?0x100:0x101,vk,lp},0};d->held[vk]=down?-32768:0;
 }else return input_error(d,"UnsupportedMacInputEvent");
 d->human_events++;return 0;
}
uint32_t macos_live_order_tags(MacOSLiveDriver*d,uint32_t*out,uint32_t capacity){
 if(!d||!out)return UINT32_MAX;RlPlayerInputEvent events[128];uint32_t n=0,count=0;
 int c=d->rl_eventos_entrada(d->world,events,128,&n);if(c){input_error(d,"InputDiagnosticsCapacityOrABIError");return UINT32_MAX;}
 for(uint32_t i=0;i<n;i++)if(events[i].kind==7){if(count==capacity){input_error(d,"OrderDiagnosticCapacityExceeded");return UINT32_MAX;}out[count++]=events[i].values[1];}
 return count;
}
uint64_t macos_live_human_events(MacOSLiveDriver*d){return d?d->human_events:0;}
uint64_t macos_live_declared_frames(MacOSLiveDriver*d){return d?d->declared_frames:0;}
int32_t macos_live_frame_context(MacOSLiveDriver*d,uint8_t*out,uint32_t capacity){if(!d||!out||capacity!=sizeof(d->input))return 1;memcpy(out,&d->input,capacity);return 0;}
int32_t macos_live_input_phase(MacOSLiveDriver*d,uint32_t phase,const MacOSDeclaredInput*declared){
 if(!d||d->validation||d->status.stopped||d->status.phase_index==UINT32_MAX||(!d->production_input&&d->status.frames<128)||!declared||declared->size!=sizeof(*declared)||phase!=(d->input_next_phase?d->input_next_phase:2))return 1;
 RlDeclaredInput input;memcpy(&input,declared,sizeof(input));
 uint64_t t=now();int c=d->rl_passo_quadro_entrada(d->world,phase,&d->input,&input);d->status.logic_ns+=now()-t;d->status.stop_phase=phase;
 if(c)return fail(d,c,phase==2?"entrada entre quadros":"entrada CalculGame");
 if(phase==2){d->cursor_x=d->input.cursor_x;d->cursor_y=d->input.cursor_y;}
 d->status.phase_index=(uint32_t)(2*d->status.frames+(phase==2?1:2));
 d->input_next_phase=phase==2?3:2;
 if(d->sound_capture)sound_collect(d,phase);
 if(phase==3){d->status.frames++;d->declared_frames++;uint64_t g=0;uint32_t p=0,counter=0;c=d->rl_render_geracao(d->world,&g,&p,&counter);if(c)return fail(d,c,"generation after input");d->status.counter=counter;}
 return 0;
}
int32_t macos_live_human_phase(MacOSLiveDriver*d,uint32_t phase){
 if(!d||d->validation||d->status.stopped||d->status.phase_index==UINT32_MAX||(!d->production_input&&d->status.frames<128))return 1;
 if(phase==3){d->frame_input.flags=6;d->frame_input.messages=NULL;d->frame_input.message_count=0;return macos_live_input_phase(d,3,&d->frame_input);}
 if(phase!=2||d->input_next_phase==3)return 1;
 /* GetCursorPos samples the retained host position on every phase2, including
  * the first one after MissionLoad. No mouse packet is required. Phase1 keeps
  * its original startup cursor and never consumes this host sample. */
 MacOSInputMessage *messages=d->frame_messages;MacOSInputKey *keys=d->frame_keys;uint32_t count=0,nkeys=0,keep=0;uint64_t frame=d->status.frames+1;uint32_t flags=7;
 for(uint32_t i=0;i<d->pending_count;i++){
  QueuedInput q=d->pending[i];if(q.frame<=frame){if(q.cursor){d->cursor_x=q.x;d->cursor_y=q.y;flags|=1;}messages[count++]=q.message;}else d->pending[keep++]=q;
 }
 d->pending_count=keep;
 for(uint32_t vk=0;vk<256;vk++)if(d->held[vk]&&(vk<48||vk>57)&&vk!=9)keys[nkeys++]=(MacOSInputKey){vk,d->held[vk]};
 d->frame_input=(MacOSDeclaredInput){sizeof(d->frame_input),flags,d->cursor_x,d->cursor_y,messages,count,keys,nkeys};
 return macos_live_input_phase(d,2,&d->frame_input);
}
void macos_live_audit_window(MacOSLiveDriver*d,uint32_t phases){if(d)d->audit_limit=phases<d->state_count?phases:d->state_count;}
int32_t macos_live_advance(MacOSLiveDriver*d,MacOSPhaseObserver observer,void*ctx){
 if(!d||d->status.stopped||d->shell)return 1;int c;uint64_t t;
 if(!d->validation){uint32_t screen=0;c=d->rl_missao_tela(d->world,&screen);if(c)return fail(d,c,"tela da campanha");if(screen==19||screen==16)return 0;}
 if(d->status.phase_index==UINT32_MAX){
  t=now();
  if(d->production_input){
   RlInputKey keys[256];uint32_t count=0;
   for(uint32_t vk=0;vk<256;vk++)if(d->held[vk])keys[count++]=(RlInputKey){vk,d->held[vk]};
   /* No message pump or cursor sample before the original phase2 boundary. */
   RlDeclaredInput declared={sizeof(declared),6,0,0,NULL,0,keys,count};
   c=d->rl_load_tail_entrada_v1(d->world,&d->input,&declared);
  }else c=d->rl_passo_quadro(d->world,1,&d->input);d->status.logic_ns+=now()-t;if(c)return fail(d,c,"load_tail");d->input.registers=d->phase2.registers;
  if(!d->validation){
   if(d->game_surface){c=first_game_draw(d);if(c)return c;}
   c=d->rl_autosalvar_inicio_fase(d->world,&d->input);if(c)return fail(d,c,"captura automática no início da fase");
   d->stage_presentation=malloc(PRESENTATION_SIZE);if(!d->stage_presentation)return fail(d,RL_IO_ERROR,"memória da apresentação automática");
   c=capture_presentation(d,d->stage_presentation);if(c)return fail(d,c,"apresentação automática");
   d->stage_cursor_x=d->cursor_x;d->stage_cursor_y=d->cursor_y;
  }
  return proof(d,0,1,observer,ctx);
 }
 uint64_t gen=0;uint32_t phase=0,counter=0;t=now();c=d->rl_render_geracao(d->world,&gen,&phase,&counter);if(c)return fail(d,c,"render generation");
 /* A world resumed from the original pause menu (rl_shell_resume) has no frame
  * phase yet: its picture is the retained gameplay presentation, and the next
  * complete frame is drawn normally. */
 if(!(phase==0&&!d->validation)){
 RlRenderResult out={0};c=d->rl_render_quadro(d->renderer,gen,1,d->pixels,sizeof(d->pixels),d->palette,sizeof(d->palette),&out);d->status.render_ns+=now()-t;
 if(c)return fail(d,c,"Rust render");
 if(out.abi_status||out.generation!=gen||out.width!=800||out.height!=600||out.stride!=800||out.required_pixels!=480000||out.required_palette!=768||out.palette_generation!=1||out.disposition>2){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"NativeRenderResultContractMismatch");return 5;}
 d->status.generation=gen;d->status.disposition=out.disposition;
 if(out.disposition==2){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"%s at 0x%08x: %s",out.stop_callee,out.stop_address,out.stop_reason);return 5;}
 }
 if(d->validation){t=now();CC_SHA256(d->pixels,sizeof(d->pixels),d->status.surface_sha);
 if(d->status.frames<d->surface_count){if(memcmp(d->status.surface_sha,d->surface_hashes+32*d->status.frames,32)){d->status.stopped=1;snprintf(d->status.message,sizeof(d->status.message),"SurfaceProofMismatch no quadro %llu",(unsigned long long)(d->status.frames+1));return 5;}d->status.surfaces_equal++;}
 d->status.proof_ns+=now()-t;}
 if(!d->validation){
  uint32_t pending=0;c=d->rl_entrada_quantidade_pendente(d->world,&pending);if(c)return fail(d,c,"consulta de entradas salvas");
  if(pending){
   /* A restored FIFO owns its phase answers. Drain to the next complete frame
    * before admitting newer host messages. A failed item remains queued. */
   do{c=d->rl_entrada_passo_pendente(d->world,&d->input);if(c)return fail(d,c,"entrada salva");
    c=d->rl_render_geracao(d->world,&gen,&phase,&counter);if(c)return fail(d,c,"fase da entrada salva");
    if(phase==2){d->cursor_x=d->input.cursor_x;d->cursor_y=d->input.cursor_y;}
    if(d->sound_capture)sound_collect(d,phase);
    if(phase==3){d->status.frames++;d->declared_frames++;d->status.counter=counter;d->status.phase_index=(uint32_t)(2*d->status.frames);d->input_next_phase=2;return 0;}
    c=d->rl_entrada_quantidade_pendente(d->world,&pending);if(c)return fail(d,c,"consulta de entradas salvas");
   }while(pending);
   return input_error(d,"SavedInputMissingPhase3");
  }
 }
 if(!d->validation&&(d->production_input||d->status.frames>=128)){c=macos_live_human_phase(d,2);return c?c:macos_live_human_phase(d,3);}
 t=now();c=d->rl_passo_quadro(d->world,2,&d->input);d->status.logic_ns+=now()-t;if(c)return fail(d,c,"between_frames");
 if((c=proof(d,(uint32_t)(2*d->status.frames+1),2,observer,ctx)))return c;
 t=now();c=d->rl_passo_quadro(d->world,3,&d->input);d->status.logic_ns+=now()-t;if(c)return fail(d,c,"CalculGame");
 if((c=proof(d,(uint32_t)(2*d->status.frames+2),3,observer,ctx)))return c;
 d->status.frames++;c=d->rl_render_geracao(d->world,&gen,&phase,&counter);if(c)return fail(d,c,"generation after frame");d->status.counter=counter;return 0;
}
void macos_live_status(MacOSLiveDriver*d,MacOSLiveStatus*out){if(d&&out){*out=d->status;out->world=d->world;uint64_t g=0;uint32_t p=0,c=0;if(d->validation&&d->rl_render_geracao(d->world,&g,&p,&c)==0)out->counter=c;}}
uint64_t macos_live_renderer(MacOSLiveDriver*d){return d?d->renderer:0;}
const uint8_t*macos_live_pixels(MacOSLiveDriver*d){return d?d->pixels:NULL;}
const uint8_t*macos_live_palette(MacOSLiveDriver*d){return d?d->palette:NULL;}
int32_t macos_live_read(MacOSLiveDriver*d,uint32_t address,uint32_t size,uint8_t*out){
 if(!d||!out||!size||!d->validation)return 1;uint32_t n=0;int c=d->rl_projetar_tudo(d->world,d->bulk,d->bulk_capacity,&n);if(c)return c;
 uint32_t off=4;for(uint32_t i=0;i<le(d->bulk)&&off+8<=n;i++){uint32_t base=le(d->bulk+off),len=le(d->bulk+off+4);off+=8;if((uint64_t)off+len>n)return 3;if(address>=base&&(uint64_t)address+size<=(uint64_t)base+len){memcpy(out,d->bulk+off+address-base,size);return 0;}off+=len;}return 3;
}
/* "Atalhos modernos": read-only snapshot in play. One complete projection, then
 * each (address,size) range is copied to out, concatenated in request order. It
 * never writes the world, its RNG/permutation cursors, input queue or clock; the
 * app calls it between frames on the engine work queue. 1: bad call/shell/
 * validation/stopped; 3: a range is outside every projected block; else ABI code. */
int32_t macos_live_peek(MacOSLiveDriver*d,const uint32_t*addresses,const uint32_t*sizes,uint32_t count,uint8_t*out){
 if(!d||!addresses||!sizes||!out||!count||d->validation||d->shell||d->status.stopped||!d->world)return 1;
 uint32_t n=0;int c=d->rl_projetar_tudo(d->world,NULL,0,&n);if(c!=RL_BUFFER_TOO_SMALL)return c?c:3;
 if(n>d->peek_capacity){uint8_t*grown=realloc(d->peek,n);if(!grown)return 1;d->peek=grown;d->peek_capacity=n;}
 c=d->rl_projetar_tudo(d->world,d->peek,d->peek_capacity,&n);if(c)return c;
 uint64_t written=0;
 for(uint32_t r=0;r<count;r++){
  uint32_t address=addresses[r],size=sizes[r],off=4,found=0;if(!size)return 1;
  for(uint32_t i=0;n>=4&&i<le(d->peek)&&off+8<=n;i++){
   uint32_t base=le(d->peek+off),len=le(d->peek+off+4);off+=8;if((uint64_t)off+len>n)return 3;
   if(address>=base&&(uint64_t)address+size<=(uint64_t)base+len){memcpy(out+written,d->peek+off+address-base,size);found=1;break;}
   off+=len;
  }
  if(!found)return 3;written+=size;
 }
 return 0;
}
/* World rules. default_rules applies to production worlds created afterwards;
 * set_rules changes the current world only before its load tail (the menu
 * world), as rl_regras_definir does; saves and later stages keep their own. */
void macos_live_default_rules(uint32_t rules){default_rules=rules;}
int32_t macos_live_set_rules(MacOSLiveDriver*d,uint32_t rules){
 if(!d||!d->world||!d->rl_regras_definir||d->validation)return 1;
 return d->rl_regras_definir(d->world,rules);
}
uint32_t macos_live_rules(MacOSLiveDriver*d){
 uint32_t rules=0;if(d&&d->world&&d->rl_regras&&d->rl_regras(d->world,&rules)==0)return rules;return 0;
}
/* Pending Shift+click waypoints of one unit (rl_fila_ordens), play only. */
int32_t macos_live_order_queue(MacOSLiveDriver*d,uint32_t unit,uint16_t*cells,uint32_t capacity,uint32_t*count){
 if(!d||!count||!d->world||!d->rl_fila_ordens||d->validation||d->shell)return 1;
 return d->rl_fila_ordens(d->world,unit,cells,capacity,count);
}
/* Inputs queued by macos_live_event and not yet delivered to a game frame. */
uint32_t macos_live_pending_input(MacOSLiveDriver*d){return d?d->pending_count:0;}
static int save_result(MacOSLiveDriver*d,int code,const char*action){
 d->save_error[0]=0;if(!code)return 0;
 uint32_t n=0;char detail[360]={0};d->rl_erro_mensagem(NULL,0,&n);
 if(n&&n<=sizeof(detail))d->rl_erro_mensagem((uint8_t*)detail,n,&n);
 snprintf(d->save_error,sizeof(d->save_error),"%s · ABI %d: %s",action,code,detail);return code;
}
void macos_live_save_error(MacOSLiveDriver*d,char*out,uint32_t capacity){if(d&&out&&capacity)snprintf(out,capacity,"%s",d->save_error);}
/* Presentation is host-owned. Keep the engine file byte-for-byte in its ABI
 * format; a content-addressed companion supplies persistent pixels/cache only.
 * Write+sync the companion first, then let the ABI atomically replace the slot.
 * Until that commit the old slot still names its old companion. */
static int view_error(MacOSLiveDriver*d){snprintf(d->save_error,sizeof(d->save_error),"Não foi possível ler ou gravar a apresentação da partida. Mantenha o arquivo .rls e seu arquivo .view juntos.");return RL_IO_ERROR;}
static int file_digest(const char*path,uint8_t*out){
 FILE*f=fopen(path,"rb");if(!f)return 1;CC_SHA256_CTX context;CC_SHA256_Init(&context);uint8_t bytes[8192];size_t n,total=0;
 while((n=fread(bytes,1,sizeof(bytes),f))){total+=n;if(total>160u*1024u*1024u){fclose(f);return 1;}CC_SHA256_Update(&context,bytes,(CC_LONG)n);}
 int bad=ferror(f);fclose(f);if(bad)return 1;CC_SHA256_Final(out,&context);return 0;
}
static int view_path(const char*path,const uint8_t*hash,char*out){
 char hex[65];for(unsigned i=0;i<32;i++)snprintf(hex+2*i,3,"%02x",hash[i]);
 return snprintf(out,4096,"%s.%s.view",path,hex)>=4096;
}
static uint8_t*read_view(MacOSLiveDriver*d,const char*path,char*mission,uint32_t*difficulty,int32_t*host_x,int32_t*host_y,int*has_host,int*code){
 *code=RL_IO_ERROR;*has_host=0;
 uint8_t hash[32],header[72],actual[32];char target[4096];
 if(file_digest(path,hash)||view_path(path,hash,target)){view_error(d);return NULL;}
 FILE*f=fopen(target,"rb");if(!f){view_error(d);return NULL;}
 int bad=fread(header,1,sizeof(header),f)!=sizeof(header);uint32_t size=0;
 int legacy=!bad&&(!memcmp(header,"RLVIEW1\n",8)||!memcmp(header,"RLVIEW2\n",8));
 if(!bad){if(!memcmp(header,"RLVIEW4\n",8))size=VIEW_SIZE;else if(!memcmp(header,"RLVIEW3\n",8)||!memcmp(header,"RLVIEW2\n",8))size=VIEW_V3_SIZE;else if(!memcmp(header,"RLVIEW1\n",8))size=PRESENTATION_SIZE;else bad=1;}
 uint8_t*data=bad?NULL:malloc(size);bad=bad||!data;
 if(!bad)bad=fread(data,1,size,f)!=size||fgetc(f)!=EOF||ferror(f);fclose(f);
 if(!bad){CC_SHA256(data,size,actual);bad=memcmp(header+8,hash,32)||memcmp(header+40,actual,32);}
 if(!bad&&le(data+751360)>255)bad=1;
 if(!bad&&size>=VIEW_V3_SIZE){
  memcpy(mission,data+PRESENTATION_SIZE,16);memcpy(difficulty,data+PRESENTATION_SIZE+16,4);
  bad=mission[15]!=0||strlen(mission)<11||strlen(mission)>12||strncmp(mission,"MONDE",5)||strcmp(mission+strlen(mission)-4,".ETA")||*difficulty>2;
 }
 if(bad){free(data);view_error(d);return NULL;}
 if(size==VIEW_SIZE){memcpy(host_x,data+VIEW_V3_SIZE,4);memcpy(host_y,data+VIEW_V3_SIZE+4,4);*has_host=1;}
 if(legacy){
  free(data);*code=RL_SAVE_VERSION;
  snprintf(d->save_error,sizeof(d->save_error),"Esta apresentação usa o HUD antigo (RLVIEW1/2). Abra a partida na versão que a salvou; os arquivos foram preservados.");return NULL;
 }
 return data;
}
/* A save is captured on the game worker (one serialisation of the paused or
 * between-frames world, its SHA-256 and the presentation companion) and then
 * committed by any thread without the driver: companion first, then the slot
 * file through a temporary + fsync + rename, so the previous save survives any
 * write failure. The slot bytes are exactly the engine's rl_salvar_bytes /
 * rl_autosalvar_bytes output, the same bytes rl_salvar_arquivo writes. */
struct MacOSSaveJob {char path[4096];uint8_t*bytes;uint32_t n;uint8_t hash[32];uint8_t header[72];uint8_t*payload;char error[512];};
void macos_live_save_job_free(MacOSSaveJob*j){if(!j)return;free(j->bytes);free(j->payload);free(j);}
static int job_view_error(MacOSSaveJob*j){snprintf(j->error,sizeof(j->error),"Não foi possível ler ou gravar a apresentação da partida. Mantenha o arquivo .rls e seu arquivo .view juntos.");return RL_IO_ERROR;}
static int durable_write(const char*target,const uint8_t*a,size_t an,const uint8_t*b,size_t bn){
 char temporary[4120];if(snprintf(temporary,sizeof(temporary),"%s.tmp.XXXXXX",target)>=(int)sizeof(temporary))return 1;
 int fd=mkstemp(temporary);if(fd<0)return 1;
 FILE*f=fdopen(fd,"wb");if(!f){close(fd);unlink(temporary);return 1;}
 int bad=(an&&fwrite(a,1,an,f)!=an)||(bn&&fwrite(b,1,bn,f)!=bn)||fflush(f)||fsync(fd);
 if(fclose(f))bad=1;if(!bad)bad=chmod(temporary,0644)!=0;
 if(bad||rename(temporary,target)){unlink(temporary);return 1;}return 0;
}
static void sync_directory(const char*path){
 char directory[4096];snprintf(directory,sizeof(directory),"%s",path);char*slash=strrchr(directory,'/');
 if(!slash)return;if(slash==directory)slash[1]=0;else*slash=0;
 int fd=open(directory,O_RDONLY);if(fd>=0){fsync(fd);close(fd);}
}
int32_t macos_live_save_commit(MacOSSaveJob*j){
 if(!j)return RL_INVALID_ARGUMENT;j->error[0]=0;
 uint8_t old_hash[32];int had_old=file_digest(j->path,old_hash)==0;
 char target[4096];if(view_path(j->path,j->hash,target))return job_view_error(j);
 if(durable_write(target,j->header,sizeof(j->header),j->payload,VIEW_SIZE))return job_view_error(j);
 if(durable_write(j->path,j->bytes,j->n,NULL,0)){
  snprintf(j->error,sizeof(j->error),"Não foi possível salvar · %s: %s. A partida salva anterior foi mantida.",j->path,strerror(errno?errno:EIO));return RL_IO_ERROR;
 }
 sync_directory(j->path);
 if(had_old&&memcmp(old_hash,j->hash,32)){char old_path[4096];if(!view_path(j->path,old_hash,old_path))unlink(old_path);}
 return 0;
}
void macos_live_save_job_error(MacOSSaveJob*j,char*out,uint32_t capacity){if(j&&out&&capacity)snprintf(out,capacity,"%s",j->error);}
static int serialize(MacOSLiveDriver*d,int stage,uint8_t**out,uint32_t*size){
 /* One serialisation in steady state: the buffer keeps the last size plus slack;
  * only a grown world repeats the call after RL_BUFFER_TOO_SMALL. */
 uint32_t capacity=d->save_capacity?d->save_capacity:4u*1024u*1024u;
 for(int attempt=0;attempt<3;attempt++){
  uint8_t*bytes=malloc(capacity);if(!bytes)return RL_IO_ERROR;uint32_t n=0;
  int c=stage?d->rl_autosalvar_bytes(d->world,bytes,capacity,&n):d->rl_salvar_bytes(d->world,&d->input,bytes,capacity,&n);
  if(!c){d->save_capacity=n+n/8+65536;*out=bytes;*size=n;return 0;}
  free(bytes);if(c!=RL_BUFFER_TOO_SMALL||n<=capacity)return c;capacity=n+n/8+65536;
 }
 return RL_SAVE_INVALID;
}
static int snapshot_capture(MacOSLiveDriver*d,const char*path,int stage,MacOSSaveJob**out){
 *out=NULL;
 if(!d->game_surface){snprintf(d->save_error,sizeof(d->save_error),"A apresentação histórica é somente para validação. Salve uma partida no modo Jogar.");return RL_SAVE_BUSY;}
 if(strlen(path)>=4000)return save_result(d,RL_INVALID_ARGUMENT,"Caminho da partida");
 /* Ordinary saves follow logic, while the retained pixels may precede it.
  * Accept this boundary's draw once before serializing world and presentation.
  * Stage autosaves already captured their accepted first draw together. */
 /* A game paused in the original menu (mode16, saved from its save list) keeps
  * the retained gameplay picture: AffGame does not draw the pause screen. */
 uint32_t screen=0;if(d->rl_missao_tela(d->world,&screen))screen=0;
 if(!stage&&screen!=16){int c=first_game_draw(d);if(c)return save_result(d,c,"Não foi possível preparar a apresentação da partida");}
 MacOSSaveJob*j=calloc(1,sizeof(*j));if(!j)return view_error(d);
 snprintf(j->path,sizeof(j->path),"%s",path);
 int c=serialize(d,stage,&j->bytes,&j->n);
 if(c){macos_live_save_job_free(j);return save_result(d,c,"Não foi possível preparar a partida");}
 CC_SHA256(j->bytes,j->n,j->hash);
 j->payload=malloc(VIEW_SIZE);if(!j->payload){macos_live_save_job_free(j);return view_error(d);}
 if(stage)memcpy(j->payload,d->stage_presentation,PRESENTATION_SIZE);
 else{c=capture_presentation(d,j->payload);if(c){macos_live_save_job_free(j);return save_result(d,c,"Não foi possível preparar a apresentação");}}
 memcpy(j->payload+PRESENTATION_SIZE,d->mission,16);memcpy(j->payload+PRESENTATION_SIZE+16,&d->difficulty,4);
 int32_t x=stage?d->stage_cursor_x:d->cursor_x,y=stage?d->stage_cursor_y:d->cursor_y;
 memcpy(j->payload+VIEW_V3_SIZE,&x,4);memcpy(j->payload+VIEW_V3_SIZE+4,&y,4);
 memcpy(j->header,"RLVIEW4\n",8);memcpy(j->header+8,j->hash,32);CC_SHA256(j->payload,VIEW_SIZE,j->header+40);
 d->save_error[0]=0;*out=j;return 0;
}
static int snapshot_file(MacOSLiveDriver*d,const char*path,int stage){
 MacOSSaveJob*j=NULL;int c=snapshot_capture(d,path,stage,&j);if(c)return c;
 c=macos_live_save_commit(j);if(c)snprintf(d->save_error,sizeof(d->save_error),"%s",j->error);else d->save_error[0]=0;
 macos_live_save_job_free(j);return c;
}
static int save_allowed(MacOSLiveDriver*d){
 if(d->validation||d->shell||d->status.stopped||d->status.phase_index==UINT32_MAX||d->input_next_phase==3){snprintf(d->save_error,sizeof(d->save_error),"Salve durante o jogo, entre quadros completos e antes de uma parada do motor.");return RL_SAVE_BUSY;}
 /* Host events not yet submitted are outside the engine format. Do not lose a
  * scheduled click: the UI retries at a later frame while the game is running. */
 if(d->pending_count){snprintf(d->save_error,sizeof(d->save_error),"Há cliques ou teclas aguardando o próximo quadro. Continue o jogo e tente novamente.");return RL_SAVE_BUSY;}
 /* A held button is engine state (0x5099ae) without its host release. */
 if(d->left_held){snprintf(d->save_error,sizeof(d->save_error),"Solte o botão do mouse antes de salvar a partida.");return RL_SAVE_BUSY;}
 return 0;
}
int32_t macos_live_save_begin(MacOSLiveDriver*d,const char*path,MacOSSaveJob**job){
 if(!d||!path||!job)return 1;*job=NULL;int c=save_allowed(d);return c?c:snapshot_capture(d,path,0,job);
}
int32_t macos_live_save(MacOSLiveDriver*d,const char*path){
 if(!d||!path)return 1;int c=save_allowed(d);return c?c:snapshot_file(d,path,0);
}
int32_t macos_live_autosave(MacOSLiveDriver*d,const char*path){if(!d||!path)return 1;return snapshot_file(d,path,1);}
int32_t macos_live_load(MacOSLiveDriver*d,const char*path){
 if(!d||!path)return 1;
 if(d->validation){snprintf(d->save_error,sizeof(d->save_error),"Carregue partidas no modo Jogar.");return RL_SAVE_BUSY;}
 char mission[16]={0};uint32_t difficulty=0;
 int view_code=0,has_host=0;int32_t host_x=0,host_y=0;
 uint8_t*view=read_view(d,path,mission,&difficulty,&host_x,&host_y,&has_host,&view_code);
 if(!view)return view_code;
 RlMundo world=0;RlRenderer renderer=0;RlFrameInput input={0};
 int c=d->rl_carregar_arquivo((const uint8_t*)path,(uint32_t)strlen(path),&world,&input);
 if(c){free(view);return save_result(d,c,"Não foi possível carregar");}
 uint64_t gen=0;uint32_t phase=0,frame=0,mode=0;
 c=d->rl_execucao_modo(world,&mode);if(!c)c=d->rl_render_geracao(world,&gen,&phase,&frame);
 if(!c&&(mode!=RL_EXECUCAO_JOGO||(phase!=1&&phase!=3))){free(view);d->rl_liberar_mundo(world);snprintf(d->save_error,sizeof(d->save_error),"A partida precisa estar no modo Jogar e em um quadro completo.");return RL_SAVE_BUSY;}
 if(!c&&d->events_host.version)c=d->rl_eventos_reassociar_v1(world,&d->events_host);
 if(!c)c=event_flc_declare(d,world);
 if(!c)c=new_renderer(d,world,view,&renderer);
 uint8_t*pixels=NULL,*palette=NULL;RlRenderResult rendered={0};
 if(!c){pixels=malloc(sizeof(d->pixels));palette=malloc(sizeof(d->palette));if(!pixels||!palette)c=RL_IO_ERROR;}
 /* A game saved from the original pause list is in mode16: AffGame does not
  * draw it; its saved picture and the packaged game palette stand until the
  * pause menu resumes play (the app reopens that menu on the next frame). */
 uint32_t paused=0;if(!c&&d->rl_missao_tela(world,&paused))paused=0;
 if(!c&&paused==16){
  if(d->cd_inputs){c=d->rl_mundo_paleta_jogo(world,palette,768);if(!c)memcpy(pixels,view,480000);}
  else{uint32_t pn=0;uint8_t*base=file(d->root,"palette.bin",&pn);
  if(!base||pn!=768)c=RL_IO_ERROR;else{memcpy(pixels,view,480000);memcpy(palette,base,768);}free(base);}
 }
 free(view);
 if(!c&&paused!=16)c=d->rl_render_quadro(renderer,gen,1,pixels,sizeof(d->pixels),palette,sizeof(d->palette),&rendered);
 if(!c&&(rendered.abi_status||rendered.disposition==2)){
  snprintf(d->save_error,sizeof(d->save_error),"Renderização da partida: %s at 0x%08x: %s",rendered.stop_callee,rendered.stop_address,rendered.stop_reason);
  free(pixels);free(palette);d->rl_render_liberar(renderer);d->rl_liberar_mundo(world);return 5;
 }
 if(c){free(pixels);free(palette);if(renderer)d->rl_render_liberar(renderer);d->rl_liberar_mundo(world);return save_result(d,c,"Não foi possível preparar a partida");}
 RlMundo old_world=d->world;RlRenderer old_renderer=d->renderer;
 if(d->shell)d->rl_shell_free(d->shell);d->shell=0;d->shell_mode=0;d->shell_request=(RlShellEvent){0};
 d->world=world;d->renderer=renderer;d->input=input;d->production_input=1;memcpy(d->mission,mission,16);d->difficulty=difficulty;
 d->game_surface=1;
 memcpy(d->pixels,pixels,sizeof(d->pixels));memcpy(d->palette,palette,sizeof(d->palette));free(pixels);free(palette);
 d->status=(MacOSLiveStatus){0};d->status.frames=frame;d->status.counter=frame;d->status.generation=gen;d->status.phase_index=frame*2;d->input_next_phase=2;
 d->pending_count=0;memset(d->held,0,sizeof(d->held));d->cursor_x=has_host?host_x:input.cursor_x;d->cursor_y=has_host?host_y:input.cursor_y;d->mouse_until=0;d->left_held=0;d->left_down_frame=0;d->sound_count=0;
 d->human_events=0;d->declared_frames=0;d->sound_dropped=0;d->sound_errors=0;
 if(old_renderer)d->rl_render_liberar(old_renderer);d->rl_liberar_mundo(old_world);d->menu_world=0;d->new_campaign=0;free(d->pause_presentation);d->pause_presentation=NULL;return save_result(d,0,"");
}
/* The original menu boundary is infrequent. Export once for its result screen,
 * rather than auditing/projecting the entire world on every play frame. */
static int campaign_byte(const uint8_t*data,uint32_t n,uint32_t address,uint8_t*out){
 if(n<4)return RL_SAVE_INVALID;uint32_t off=4;
 for(uint32_t i=0;i<le(data);i++){
  if((uint64_t)off+8>n)return RL_SAVE_INVALID;uint32_t base=le(data+off),len=le(data+off+4);off+=8;
  if((uint64_t)off+len>n)return RL_SAVE_INVALID;
  if(address>=base&&(uint64_t)address<(uint64_t)base+len){*out=data[off+address-base];return 0;}off+=len;
 }
 return RL_MEMORY_ERROR;
}
static int next_path(MacOSLiveDriver*d,uint8_t*path,uint32_t*size,char*name){
 int c=d->rl_missao_nome_proxima(d->world,path,256,size);if(c)return c;
 if(!*size||*size>=256)return RL_INVALID_ARGUMENT;path[*size]=0;
 const char*base=(const char*)path;for(uint32_t i=0;i<*size;i++)if(path[i]=='/'||path[i]==92)base=(const char*)path+i+1;
 if(strlen(base)<11||strlen(base)>12)return RL_INVALID_ARGUMENT;
 snprintf(name,16,"%s",base);for(unsigned i=0;name[i];i++)if(name[i]>='a'&&name[i]<='z')name[i]-='a'-'A';
 if(strncmp(name,"MONDE",5)||strcmp(name+strlen(name)-4,".ETA"))return RL_INVALID_ARGUMENT;
 return 0;
}
int32_t macos_live_campaign_screen(MacOSLiveDriver*d,uint32_t*out){
 if(!d||!out||d->validation)return RL_INVALID_ARGUMENT;
 return d->rl_missao_tela(d->world,out);
}
int32_t macos_live_campaign_end(MacOSLiveDriver*d,MacOSCampaignEnd*out){
 if(!d||!out||d->validation)return RL_INVALID_ARGUMENT;
 uint32_t n=0;int c=d->rl_projetar_tudo(d->world,NULL,0,&n);if(c!=RL_BUFFER_TOO_SMALL)return save_result(d,c?c:RL_SAVE_INVALID,"Resultado da fase");
 uint8_t*data=malloc(n);if(!data)return save_result(d,RL_IO_ERROR,"Resultado da fase");
 c=d->rl_projetar_tudo(d->world,data,n,&n);uint8_t end=0,victory=0,stage=0;
 if(!c)c=campaign_byte(data,n,0x4e05a0,&end);
 if(!c)c=campaign_byte(data,n,0x4e1d84,&victory);
 if(!c)c=campaign_byte(data,n,0x496fe4,&stage);free(data);
 if(c)return save_result(d,c,"Resultado da fase");
 MacOSCampaignEnd result={0};result.stage=stage;result.outcome=end?(victory?1:2):0;
 if(result.outcome==1){
  uint8_t path[257];uint32_t length=0;c=next_path(d,path,&length,result.next);
  if(c)return save_result(d,c,"Próxima fase");
  uint32_t bytes=0;uint8_t*eta=file(d->root,result.next,&bytes);result.has_next=eta!=NULL;free(eta);
  if(!result.has_next&&stage!=25){snprintf(d->save_error,sizeof(d->save_error),"Arquivo da próxima fase ausente: %s",result.next);return RL_IO_ERROR;}
 }
 *out=result;return 0;
}
static int new_campaign_path(MacOSLiveDriver*d,uint8_t*path,uint32_t*size,char*name);
static int continue_stage(MacOSLiveDriver*d,uint32_t difficulty,int prepared){
 if(!d||d->validation||(!prepared&&d->shell)||difficulty>2)return RL_INVALID_ARGUMENT; /* presets 0..2 only, before any engine call */
 if(d->status.stopped&&(!strstr(d->status.message,"MenuInit mission end")||(!strstr(d->status.message,"0x0042c931")&&!strstr(d->status.message,"0x42c931")))){snprintf(d->save_error,sizeof(d->save_error),"A parada atual do motor precisa ser resolvida antes de continuar.");return RL_UNSUPPORTED_PATH;}
 uint8_t path[257];uint32_t length=0;char name[16];int c=0;
 /* A completed original "Novo jogo" loads stage 1; every other continuation
  * still requires an actual victory with an available next stage. */
 int fresh=prepared&&d->new_campaign;
 if(fresh){c=new_campaign_path(d,path,&length,name);if(c)return save_result(d,c,"Nova campanha");}
 else{
 MacOSCampaignEnd end={0};c=macos_live_campaign_end(d,&end);if(c)return c;
 if(end.outcome!=1||!end.has_next){snprintf(d->save_error,sizeof(d->save_error),"Não há vitória com uma próxima fase disponível.");return RL_UNSUPPORTED_PATH;}
 uint32_t queued=0;c=d->rl_entrada_quantidade_pendente(d->world,&queued);
 if(c)return save_result(d,c,"Entradas da fase anterior");
 if(queued){snprintf(d->save_error,sizeof(d->save_error),"Há entradas salvas pendentes na fase anterior.");return RL_SAVE_BUSY;}
 c=next_path(d,path,&length,name);if(c)return save_result(d,c,"Próxima fase");
 }
 uint32_t eta_length=0,monsters_length=0;uint8_t*eta=file(d->root,name,&eta_length),*monsters=file(d->root,"MONSTRE.INI",&monsters_length);
 if(!eta||!monsters){free(eta);free(monsters);snprintf(d->save_error,sizeof(d->save_error),"Dados originais da próxima fase ausentes: %s / MONSTRE.INI",name);return RL_IO_ERROR;}
 MacOSLiveDriver*candidate=malloc(sizeof(*candidate));if(!candidate){free(eta);free(monsters);return save_result(d,RL_IO_ERROR,"Próxima fase");}
 *candidate=*d;snprintf(candidate->mission,16,"%s",name);candidate->difficulty=difficulty;candidate->world=0;candidate->renderer=0;candidate->stage_presentation=NULL;
 candidate->menu_world=0;candidate->new_campaign=0;candidate->pause_presentation=NULL;
 candidate->shell=0;candidate->shell_generation=0;candidate->shell_mode=0;candidate->shell_request=(RlShellEvent){0};
 candidate->save_error[0]=0;
 candidate->status=(MacOSLiveStatus){0};candidate->status.phase_index=UINT32_MAX;
 candidate->pending_count=0;candidate->input_next_phase=2;if(!candidate->production_input)memset(candidate->held,0,sizeof(candidate->held));candidate->mouse_until=0;candidate->left_held=0;candidate->left_down_frame=0;
 candidate->human_events=0;candidate->declared_frames=0;candidate->sound_count=0;candidate->sound_dropped=0;candidate->sound_errors=0;
 if(prepared){
  RlFrameInput carried=d->input;carried.cursor_x=(int32_t)d->shell_input.mouse_x;carried.cursor_y=(int32_t)d->shell_input.mouse_y;
  candidate->cursor_x=carried.cursor_x;candidate->cursor_y=carried.cursor_y;memset(candidate->held,0,sizeof(candidate->held));
  /* Load and stage-start preset in one engine transaction; no separate application. */
  c=d->rl_shell_campaign_load_dificuldade(d->shell,d->shell_generation,(RlShellBytes){path,length},(RlShellBytes){eta,eta_length},(RlShellBytes){monsters,monsters_length},&carried,difficulty,&candidate->world,&candidate->input);
  /* The menu world carries no play execution mode: set it before the load tail. */
  if(!c&&d->menu_world)c=d->rl_execucao_definir(candidate->world,RL_EXECUCAO_JOGO);
 }else c=d->rl_missao_continuar(d->world,path,length,eta,eta_length,monsters,monsters_length,&d->input,&candidate->world,&candidate->input);
 /* Both continuation APIs carry the last sampled original cursor. The next
  * stage drops unsent old-stage input and starts its host hold from that same
  * context, just as loading its stage autosave does. */
 if(!c){candidate->cursor_x=candidate->input.cursor_x;candidate->cursor_y=candidate->input.cursor_y;}
 free(eta);free(monsters);
 /* Normal is the exact loaded bytes. Non-normal presets must be applied by
  * the engine to this fresh stage, never approximated by host memory edits. */
 if(!c&&!prepared&&difficulty!=RL_DIFICULDADE_NORMAL)c=d->rl_aplicar_dificuldade(candidate->world,difficulty);
 if(!c)c=load_events(candidate,candidate->world,1);
 if(!c)c=new_renderer(candidate,candidate->world,NULL,&candidate->renderer);
 if(!c)c=macos_live_advance(candidate,NULL,NULL);
 uint64_t generation=0;uint32_t phase=0,frame=0;RlRenderResult rendered={0};int render_error=0;
 if(!c)c=d->rl_render_geracao(candidate->world,&generation,&phase,&frame);
 if(!c)c=d->rl_render_quadro(candidate->renderer,generation,1,candidate->pixels,sizeof(candidate->pixels),candidate->palette,sizeof(candidate->palette),&rendered);
 if(!c&&(rendered.generation!=generation||rendered.width!=800||rendered.height!=600||rendered.stride!=800||rendered.required_pixels!=480000||rendered.required_palette!=768||rendered.palette_generation!=1||rendered.disposition>2)){
  snprintf(d->save_error,sizeof(d->save_error),"Contrato de renderização inválido na próxima fase.");c=RL_UNSUPPORTED_PATH;render_error=1;
 }
 if(!c&&(rendered.abi_status||rendered.disposition==2)){
  snprintf(d->save_error,sizeof(d->save_error),"Próxima fase: %s at 0x%08x: %s",rendered.stop_callee,rendered.stop_address,rendered.stop_reason);c=RL_UNSUPPORTED_PATH;
  render_error=1;
 }else if(!c)c=capture_presentation(candidate,candidate->stage_presentation);
 if(c){
  if(candidate->status.stopped)snprintf(d->save_error,sizeof(d->save_error),"%s",candidate->status.message);
  else if(!render_error){if(candidate->save_error[0])snprintf(d->save_error,sizeof(d->save_error),"%s",candidate->save_error);else save_result(d,c,"Não foi possível continuar a campanha");}
  if(candidate->renderer)d->rl_render_liberar(candidate->renderer);if(candidate->world)d->rl_liberar_mundo(candidate->world);free(candidate->stage_presentation);free(candidate);return c;
 }
 candidate->status.generation=generation;candidate->status.disposition=rendered.disposition;
 if(d->shell)d->rl_shell_free(d->shell);
 if(d->renderer)d->rl_render_liberar(d->renderer);d->rl_liberar_mundo(d->world);free(d->stage_presentation);free(d->pause_presentation);
 *d=*candidate;free(candidate);d->save_error[0]=0;return 0;
}
int32_t macos_live_continue(MacOSLiveDriver*d,uint32_t difficulty){return continue_stage(d,difficulty,0);}
/* Preset of every later stage start; a stage already loaded keeps its own. */
int32_t macos_live_set_difficulty(MacOSLiveDriver*d,uint32_t difficulty){
 if(!d||d->validation||difficulty>2)return RL_INVALID_ARGUMENT;
 d->difficulty=difficulty;return 0;
}
int32_t macos_live_selection(MacOSLiveDriver*d,char*mission,uint32_t capacity,uint32_t*difficulty){
 if(!d||!mission||capacity<16||!difficulty)return RL_INVALID_ARGUMENT;snprintf(mission,capacity,"%s",d->mission);*difficulty=d->difficulty;return 0;
}
void macos_live_free(MacOSLiveDriver*d){if(!d)return;if(d->options&&d->rl_opcoes_liberar)d->rl_opcoes_liberar(d->options);if(d->shell&&d->rl_shell_free)d->rl_shell_free(d->shell);if(d->renderer&&d->rl_render_liberar)d->rl_render_liberar(d->renderer);if(d->world&&d->rl_liberar_mundo)d->rl_liberar_mundo(d->world);if(d->library)dlclose(d->library);free(d->events_directory);free(d->events_bytes);free(d->root);free(d->stage_presentation);free(d->pause_presentation);free(d->texts);free(d->bulk);free(d->peek);free(d->surface_hashes);free(d->state_hashes);free(d->entries);free(d->spans);free(d->events);free(d);}

MacOSLiveDriver *macos_live_create_mission_host_v1(const char*library,const char*root,const char*name,uint32_t difficulty,const uint8_t*logical,uint32_t length,char*error,uint32_t capacity){
 if(!root||!logical||!length){if(error&&capacity)snprintf(error,capacity,"Mapeamento explícito de EVENTS ausente");return NULL;}
 int fd=open(root,O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
 if(fd<0){if(error&&capacity)snprintf(error,capacity,"Diretório de dados nativos indisponível");return NULL;}
 RlOwnedEventsV1 owned={0};int code=rl_events_file_read_v1(fd,logical,length,&owned);close(fd);
 if(code){if(error&&capacity)snprintf(error,capacity,"EVENTS.INI original ausente ou inválido (%d)",code);return NULL;}
 MacOSLiveDriver*d=macos_live_create_mission_eventos_v1(library,root,name,difficulty,&owned.input,error,capacity);
 rl_events_file_free_v1(&owned);return d;
}

/* Original main menu start (no stage). Same authorized EVENTS directory contract. */
MacOSLiveDriver *macos_live_create_menu_host_v1(const char*library,const char*root,uint32_t difficulty,const uint8_t*logical,uint32_t length,char*error,uint32_t capacity){
 if(!root||!logical||!length||difficulty>2){if(error&&capacity)snprintf(error,capacity,"Mapeamento explícito de EVENTS ausente");return NULL;}
 int fd=open(root,O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
 if(fd<0){if(error&&capacity)snprintf(error,capacity,"Diretório de dados nativos indisponível");return NULL;}
 RlOwnedEventsV1 owned={0};int code=rl_events_file_read_v1(fd,logical,length,&owned);close(fd);
 if(code){if(error&&capacity)snprintf(error,capacity,"EVENTS.INI original ausente ou inválido (%d)",code);return NULL;}
 MacOSLiveDriver*d=create(library,root,NULL,difficulty,&owned.input,error,capacity,1);
 rl_events_file_free_v1(&owned);return d;
}

/* Pure query of the last committed phase. Call once after a successful advance;
 * failed transactions retain prior events and MUST NOT be replayed by the host. */
int32_t macos_live_cd_events(MacOSLiveDriver*d,RlFrameEvent*out,uint32_t capacity,uint32_t*required){
 if(!d||d->validation||!required)return RL_INVALID_ARGUMENT;
 return d->rl_eventos_quadro(d->world,out,capacity,required);
}

#include "macos_campaign_shell.inc"
#include "macos_event_flc.inc"
#include "macos_options.inc"
