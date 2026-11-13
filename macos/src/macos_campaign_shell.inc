/* Private C consumer of the prepared-shell ABI. No game rules live here. */
static int shell_copy_picture(MacOSLiveDriver*d){
 uint8_t*p=malloc(480000+768);if(!p)return RL_IO_ERROR;
 int c=d->rl_shell_copy(d->shell,d->shell_generation,p,480000,p+480000,768);
 if(!c){memcpy(d->pixels,p,480000);memcpy(d->palette,p+480000,768);}free(p);
 /* Port-only "Opções" button over main 2 / pause 16 (no-op elsewhere). It draws
  * on this copy only; a failure leaves the original picture as it is. */
 if(!c&&d->options&&d->rl_opcoes_sobrepor)(void)d->rl_opcoes_sobrepor(d->options,d->shell,d->pixels,480000);
 return c;
}
static int shell_paint(MacOSLiveDriver*d){
 RlShellPlan plan={0};int c=d->rl_shell_plan(d->shell,d->shell_mode,&plan);if(c)return c;
 /* The background is decoded from the CD's own file (imported unchanged). */
 const char*name=NULL;
 if(!strcmp((char*)plan.background,"\\ecranfin.lzs"))name="ECRANFIN.LZS";
 else if(!strcmp((char*)plan.background,"\\pagemis2.pie"))name="PAGEMIS2.PIE";
 else if(!strcmp((char*)plan.background,"\\pagemis.pie"))name="PAGEMIS.PIE";
 else if(!strcmp((char*)plan.background,"\\titre.pie"))name="TITRE.PIE";
 else if(!strcmp((char*)plan.background,"\\credits.pie"))name="CREDITS.PIE";
 if(!name)return RL_UNSUPPORTED_PATH;
 uint32_t fn=0,n=480000;uint8_t*f=file(d->root,name,&fn),*p=malloc(480000);
 c=f&&p?d->rl_shell_imagem_cd(f,fn,p,480000):RL_IO_ERROR;free(f);
 if(c){free(p);snprintf(d->save_error,sizeof(d->save_error),"Fundo original ausente ou inválido: %s",name);return RL_IO_ERROR;}
 c=d->rl_shell_paint_prepared(d->shell,d->shell_generation,(RlShellBytes){p,n},&d->shell_generation);free(p);
 return c?c:shell_copy_picture(d);
}
static int shell_world_bytes(MacOSLiveDriver*d,uint8_t**out,uint32_t*n){
 int c=d->rl_projetar_tudo(d->world,NULL,0,n);if(c!=RL_BUFFER_TOO_SMALL)return c?c:RL_SAVE_INVALID;
 *out=malloc(*n);if(!*out)return RL_IO_ERROR;
 c=d->rl_projetar_tudo(d->world,*out,*n,n);if(c){free(*out);*out=NULL;}return c;
}
static int shell_word(const uint8_t*b,uint32_t n,uint32_t address,uint32_t size,uint32_t*out){
 uint32_t value=0;for(uint32_t i=0;i<size;i++){uint8_t v=0;int c=campaign_byte(b,n,address+i,&v);if(c)return c;value|=(uint32_t)v<<(8*i);}*out=value;return 0;
}
static int shell_string(const uint8_t*b,uint32_t n,uint32_t address,uint8_t*out,uint32_t cap,uint32_t*length){
 for(uint32_t i=0;i<cap;i++){int c=campaign_byte(b,n,address+i,&out[i]);if(c)return c;if(!out[i]){*length=i;return 0;}}return RL_INVALID_ARGUMENT;
}
/* The live players table [0x5080a8] (PlayerRetrieve 0x46df41): four 20-byte records,
 * +6 alliance mask, +0x0c..+0x13 four signed-16 sympathies. Diagnostic read only. */
int32_t macos_live_players(MacOSLiveDriver*d,uint8_t*out,uint32_t capacity){
 if(!d||!d->world||!out||capacity<80)return RL_INVALID_ARGUMENT;
 uint8_t*b=NULL;uint32_t n=0,table=0;int c=shell_world_bytes(d,&b,&n);if(c)return c;
 c=shell_word(b,n,0x5080a8,4,&table);
 for(uint32_t i=0;!c&&i<80;i++)c=campaign_byte(b,n,table+i,&out[i]);
 free(b);return c;
}
uint32_t macos_live_shell_mode(MacOSLiveDriver*d){return d?d->shell_mode:0;}
int32_t macos_live_shell_state(MacOSLiveDriver*d,RlShellInput*out){
 if(!d||!d->shell||!out)return RL_INVALID_ARGUMENT;*out=d->shell_input;out->keys=0;out->mouse_b0=0;return 0;
}
int32_t macos_live_shell_begin(MacOSLiveDriver*d){
 if(!d||d->validation||d->shell||d->status.stopped)return RL_INVALID_ARGUMENT;
 /* Result19, or pause16 entered in game by the original InputGame Escape
  * (MenuInit 16 already ran in the world; the session paints titre.pie). */
 uint32_t mode=0;int c=d->rl_missao_tela(d->world,&mode);if(c||(mode!=19&&mode!=RL_SHELL_PAUSE))return c?c:RL_INVALID_ARGUMENT;
 if(mode==RL_SHELL_PAUSE){
  /* Keep the gameplay surface/cache: resume rebuilds the renderer from it. */
  free(d->pause_presentation);d->pause_presentation=malloc(PRESENTATION_SIZE);
  if(!d->pause_presentation)return save_result(d,RL_IO_ERROR,"Menu de pausa original");
  c=capture_presentation(d,d->pause_presentation);
  if(c){free(d->pause_presentation);d->pause_presentation=NULL;return save_result(d,c,"Menu de pausa original");}
  memcpy(d->pause_palette,d->palette,768);
 }
 c=shell_open(d,mode);return c?save_result(d,c,mode==19?"Tela original de resultado":"Menu de pausa original"):0;
}
/* Prepared session over the current world (result19 or the menu world's main2). */
static int shell_open(MacOSLiveDriver*d,uint32_t mode){
 RlShellBytes assets[11]={0};uint8_t*owned[11]={0};uint32_t n=0;char name[40];RlShell shell=0;int c=0;
 for(unsigned i=0;i<11;i++){
  if(i<9)snprintf(name,sizeof(name),"asset%u.bin",i);else snprintf(name,sizeof(name),"%s",i==9?"MENU.CMP":"INVENTE.CMP");
  if(d->cd_inputs&&(i==7||i==8)){
   /* Shadow and lighting tables: computed by the world from the CD palette. */
   if(i==7){owned[7]=malloc(256);owned[8]=malloc(256);if(!owned[7]||!owned[8]){c=RL_IO_ERROR;goto done;}
    c=d->rl_mundo_tabelas_sombra_luz(d->world,owned[7],owned[8]);if(c)goto done;}
   assets[i]=(RlShellBytes){owned[i],256};continue;
  }
  owned[i]=file(d->root,name,&n);assets[i]=(RlShellBytes){owned[i],n};if(!owned[i]){c=RL_IO_ERROR;goto done;}
 }
 c=d->rl_shell_create(1,d->world,assets,11,(RlShellBytes){d->pixels,480000},&shell);
 if(!c){
  uint8_t*b=NULL;uint32_t size=0;RlShellInput input={.version=1};c=shell_world_bytes(d,&b,&size);
  if(!c)c=shell_word(b,size,0x508f7c,4,&input.clock_7c);
  if(!c)c=shell_word(b,size,0x508f84,4,&input.animation_84);
  if(!c)c=shell_word(b,size,0x5099a8,2,&input.mouse_x);
  if(!c)c=shell_word(b,size,0x5099aa,2,&input.mouse_y);free(b);
  if(!c){d->shell=shell;d->shell_mode=mode;d->shell_generation=0;d->shell_input=input;d->shell_request=(RlShellEvent){0};if(d->options&&d->rl_opcoes_fechar)(void)d->rl_opcoes_fechar(d->options);d->options_open=0;c=shell_paint(d);}
 }
done:
 for(unsigned i=0;i<11;i++)free(owned[i]);
 if(c){if(shell)d->rl_shell_free(shell);d->shell=0;d->shell_mode=0;}return c;
}
static int shell_text(MacOSLiveDriver*d,const char*name,uint8_t**out,uint32_t*size);
/* Original briefing text for stage `number` at its exact CD-root path:
 * root0x509594 + template0x498430 with the stage digits. Absent file = no entry. */
static int briefing_file(MacOSLiveDriver*d,unsigned number,uint8_t*path,uint32_t*pn,uint8_t**text,uint32_t*tn){
 uint8_t*world=NULL,relative[20];uint32_t wn=0,rn=0;*text=NULL;*tn=0;
 int c=shell_world_bytes(d,&world,&wn);
 if(!c)c=shell_string(world,wn,0x509594,path,256,pn);
 if(!c)c=shell_string(world,wn,0x498430,relative,19,&rn);free(world);
 if(!c&&(rn!=18||memcmp(relative,"\\World\\monde",12)||memcmp(relative+14,".TXT",4)))c=RL_INVALID_ARGUMENT;
 if(c)return c;
 relative[12]=(uint8_t)('0'+number/10);relative[13]=(uint8_t)('0'+number%10);memcpy(path+*pn,relative,rn);*pn+=rn;
 char name[16];snprintf(name,sizeof(name),"MONDE%02u.TXT",number);
 c=shell_text(d,name,text,tn);
 if(c)snprintf(d->save_error,sizeof(d->save_error),"Falha ao ler o briefing original %s",name);
 return c;
}
/* Exact MakeMissionName path under the CD root (0x509594 + 0x494b27 with the
 * stage digits + the variant string behind 0x497154 + 0x494b36): stage 1 for
 * "Novo jogo"; the current stage and variant for the pause menu's restart. */
static int new_campaign_path(MacOSLiveDriver*d,uint8_t*path,uint32_t*size,char*name){
 unsigned number=(unsigned)d->new_campaign;if(number<1||number>25)return RL_INVALID_ARGUMENT;
 int variant=number!=1&&strlen(d->mission)==12?d->mission[7]-'A':-1;
 uint8_t*world=NULL,prefix[32],suffix[16],extra[16];uint32_t wn=0,rn=0,pn=0,sn=0,en=0,pointer=0;
 int c=shell_world_bytes(d,&world,&wn);
 if(!c)c=shell_string(world,wn,0x509594,path,200,&rn);
 if(!c)c=shell_string(world,wn,0x494b27,prefix,31,&pn);
 if(!c)c=shell_string(world,wn,0x494b36,suffix,15,&sn);
 if(!c&&variant>=0){if(variant>3)c=RL_INVALID_ARGUMENT;if(!c)c=shell_word(world,wn,0x497154+4*(unsigned)variant,4,&pointer);if(!c)c=shell_string(world,wn,pointer,extra,15,&en);}
 free(world);
 if(!c&&pn!=14)c=RL_INVALID_ARGUMENT;
 if(c)return c;
 prefix[12]=(uint8_t)('0'+number/10);prefix[13]=(uint8_t)('0'+number%10);
 memcpy(path+rn,prefix,pn);memcpy(path+rn+pn,extra,en);memcpy(path+rn+pn+en,suffix,sn);*size=rn+pn+en+sn;path[*size]=0;
 const char*base=(const char*)path;for(uint32_t i=0;i<*size;i++)if(path[i]=='/'||path[i]==92)base=(const char*)path+i+1;
 if(strlen(base)<11||strlen(base)>12)return RL_INVALID_ARGUMENT;
 snprintf(name,16,"%s",base);for(unsigned i=0;name[i];i++)if(name[i]>='a'&&name[i]<='z')name[i]-='a'-'A';
 return strcmp(name,number==1?"MONDE01.ETA":d->mission)?RL_INVALID_ARGUMENT:0;
}
int32_t macos_live_shell_input(MacOSLiveDriver*d,const RlShellInput*input,RlShellEvent*out){
 if(!d||!d->shell||!input||!out||d->shell_request.kind)return RL_INVALID_ARGUMENT;
 RlShellEvent event={0};int c=d->rl_shell_input(d->shell,d->shell_generation,input,&event);
 if(c)return save_result(d,c,"Entrada na tela original");
 d->shell_generation=event.generation;d->shell_input=*input;d->shell_request=event;
 *out=event;return 0;
}
int32_t macos_live_shell_draw(MacOSLiveDriver*d,uint32_t animation){
 if(!d||!d->shell||d->shell_request.kind)return RL_INVALID_ARGUMENT;
 int c=d->rl_shell_draw(d->shell,d->shell_generation,animation,&d->shell_generation);
 if(!c){d->shell_input.animation_84=animation;c=shell_copy_picture(d);}return c;
}
/* Empty briefing is valid; ENOENT is absence, all other host failures are errors. */
static int shell_text(MacOSLiveDriver*d,const char*name,uint8_t**out,uint32_t*size){
 char path[4096];if(snprintf(path,sizeof(path),"%s/%s",d->root,name)>=(int)sizeof(path))return RL_IO_ERROR;
 FILE*f=fopen(path,"rb");if(!f)return errno==ENOENT?0:RL_IO_ERROR;
 if(fseek(f,0,SEEK_END)){fclose(f);return RL_IO_ERROR;}long n=ftell(f);rewind(f);
 if(n<0||n>16*1024*1024){fclose(f);return RL_IO_ERROR;}
 uint8_t*p=malloc(n?(size_t)n:1);if(!p){fclose(f);return RL_IO_ERROR;}
 int bad=fread(p,1,(size_t)n,f)!=(size_t)n||ferror(f);if(fclose(f))bad=1;
 if(bad){free(p);return RL_IO_ERROR;}*out=p;*size=(uint32_t)n;return 0;
}
static int stage_number(MacOSLiveDriver*d,unsigned*number);
int32_t macos_live_shell_prepare(MacOSLiveDriver*d){
 if(!d||!d->shell||d->shell_request.kind!=RL_SHELL_MISSION_NEXT)return RL_INVALID_ARGUMENT;
 uint8_t path[257],text_path[512],relative[20],*world=NULL;uint32_t length=0,wn=0,pn=0,rn=0;char name[16];
 int c=next_path(d,path,&length,name);if(c)return save_result(d,c,"Próxima fase");
 uint32_t en=0,tn=0;uint8_t*eta=file(d->root,name,&en);char text_name[16];snprintf(text_name,sizeof(text_name),"MONDE%c%c.TXT",name[5],name[6]);
 /* Packaged original CD files are explicitly mapped to the retained CD-root
  * namespace. No basename search, installation-root substitution or I/O masking. */
 uint8_t*text=NULL;c=shell_text(d,text_name,&text,&tn);
 /* After the last stage the original MissionNext finds no next ETA: campaign
  * end (mode24). Any other missing stage file is a packaging error, retained. */
 unsigned stage=0;int final=!eta&&!c&&!stage_number(d,&stage)&&stage==25;
 if((!eta&&!final)||c){free(eta);free(text);snprintf(d->save_error,sizeof(d->save_error),"Falha ao ler dados originais do briefing: %s / %s",name,text_name);return RL_IO_ERROR;}
 c=shell_world_bytes(d,&world,&wn);
 if(!c)c=shell_string(world,wn,0x509594,text_path,256,&pn);
 if(!c)c=shell_string(world,wn,0x498430,relative,19,&rn);free(world);
 if(!c&&(rn!=18||memcmp(relative,"\\World\\monde",12)||memcmp(relative+14,".TXT",4)))c=RL_INVALID_ARGUMENT;
 if(!c){
  relative[12]=name[5];relative[13]=name[6];memcpy(text_path+pn,relative,rn);pn+=rn;
  RlShellFile files[2]={{{path,length},{eta,en}},{{text_path,pn},{text,tn}}};
  const RlShellFile*vfs=final?files+1:files;uint32_t count=final?(text?1:0):(text?2:1);
  c=d->rl_shell_campaign_prepare(d->shell,d->shell_generation,(RlShellBytes){path,length},vfs,count,&d->shell_generation);
  if(!c){d->shell_mode=final?RL_SHELL_FINAL:17;d->shell_request=(RlShellEvent){0};c=shell_paint(d);}
 }
 free(eta);free(text);return c?save_result(d,c,"Preparação do briefing original"):0;
}
int32_t macos_live_shell_load(MacOSLiveDriver*d){
 if(!d||!d->shell||d->shell_request.kind!=RL_SHELL_START_MISSION)return RL_INVALID_ARGUMENT;
 return continue_stage(d,d->difficulty,1);
}

int32_t macos_live_shell_movie_finish(MacOSLiveDriver*d,uint32_t completion){
 if(!d||!d->shell||(d->shell_request.kind!=RL_SHELL_DEFEAT_MOVIE&&d->shell_request.kind!=RL_SHELL_CAMPAIGN_MOVIE&&d->shell_request.kind!=RL_SHELL_INTRODUCTION))return RL_INVALID_ARGUMENT;
 int c=d->rl_shell_defeat_finish(d->shell,d->shell_generation,completion,&d->shell_generation);
 if(!c){
  d->shell_mode=RL_SHELL_MAIN;d->shell_request=(RlShellEvent){0};
  d->shell_input.keys=0;d->shell_input.mouse_ae=0;d->shell_input.mouse_af=0;d->shell_input.mouse_b0=0;
  c=shell_paint(d);
 }
 return c?save_result(d,c,"Retorno original após o filme"):0;
}

/* The single connection point for the engine's general preparation (lane B,
 * main ff0a2003): every SHOW is completed by rl_shell_menu_init, "Novo jogo"
 * (RESET_CAMPAIGN 17) by rl_shell_campaign_reset. The original save/load lists
 * (12/13/14, and RESET_CAMPAIGN 13) are not yet painted by the prepared shell:
 * the app routes them to its modern load menu and never completes them here.
 * RESUME stays an explicit stop. Failures leave session, request and world. */
static int shell_entered(MacOSLiveDriver*d,uint32_t mode){
 d->shell_mode=mode;d->shell_request=(RlShellEvent){0};
 d->shell_input.keys=0;d->shell_input.mouse_ae=0;d->shell_input.mouse_af=0;d->shell_input.mouse_b0=0;
 return shell_paint(d);
}
int32_t macos_live_shell_slots(MacOSLiveDriver*d,const uint8_t*table,uint32_t length){
 if(!d||!table||length!=sizeof(d->slots))return RL_INVALID_ARGUMENT;
 for(unsigned i=0;i<10;i++)if(table[33*i]>1)return RL_INVALID_ARGUMENT;
 memcpy(d->slots,table,sizeof(d->slots));return 0;
}
/* Original slot VFS for GameSaveScanNames: the ten exact paths stored behind
 * 0x497128; a present host save is "SAV3" + its 32-byte caption, an empty
 * slot has no entry (the original absent-file caption). No legacy .sav data. */
typedef struct {RlShellFile files[10];uint8_t paths[10][256],headers[10][36];uint32_t count;} SlotFiles;
static int slot_files(MacOSLiveDriver*d,SlotFiles*out){
 uint8_t*world=NULL;uint32_t wn=0;int c=shell_world_bytes(d,&world,&wn);out->count=0;
 for(unsigned i=0;!c&&i<10;i++){
  uint32_t pointer=0,length=0;c=shell_word(world,wn,0x497128+4*i,4,&pointer);
  if(!c)c=shell_string(world,wn,pointer,out->paths[i],255,&length);
  if(c||!d->slots[i][0])continue;
  memcpy(out->headers[i],"SAV3",4);memcpy(out->headers[i]+4,d->slots[i]+1,32);
  out->files[out->count++]=(RlShellFile){{out->paths[i],length},{out->headers[i],36}};
 }
 free(world);return c;
}
static int is_list(uint32_t mode){return mode==RL_SHELL_LOAD_LIST||mode==RL_SHELL_LOAD_MENU||mode==RL_SHELL_SAVE_LIST;}
static int stage_number(MacOSLiveDriver*d,unsigned*number){
 uint8_t*world=NULL;uint32_t wn=0,value=0;int c=shell_world_bytes(d,&world,&wn);
 if(!c)c=shell_word(world,wn,0x496fe4,1,&value);free(world);
 if(!c&&(value<1||value>25))c=RL_INVALID_ARGUMENT;if(!c)*number=value;return c;
}
/* RESUME and, after the host's save, SAVE_SLOT: original in-game MenuInit(18)
 * (rl_shell_resume) returns the resumed world (mode18, same clock and frame);
 * its renderer is rebuilt from the gameplay presentation retained at pause
 * entry. Failure publishes nothing and keeps the session and request. */
static int shell_resume(MacOSLiveDriver*d){
 if(!d->pause_presentation){snprintf(d->save_error,sizeof(d->save_error),"Retorno do menu de pausa sem a apresentação do jogo");return RL_INVALID_ARGUMENT;}
 RlMundo world=0;RlRenderer renderer=0;RlFrameInput input={0};
 int c=d->rl_shell_resume(d->shell,d->shell_generation,&d->input,&world,&input);
 if(!c&&d->events_host.version)c=d->rl_eventos_reassociar_v1(world,&d->events_host);
 if(!c)c=event_flc_declare(d,world);
 if(!c)c=new_renderer(d,world,d->pause_presentation,&renderer);
 if(c){if(renderer)d->rl_render_liberar(renderer);if(world)d->rl_liberar_mundo(world);return save_result(d,c,"Retorno do menu de pausa");}
 d->rl_shell_free(d->shell);d->shell=0;d->shell_mode=0;d->shell_request=(RlShellEvent){0};
 if(d->renderer)d->rl_render_liberar(d->renderer);d->rl_liberar_mundo(d->world);
 d->world=world;d->renderer=renderer;d->input=input;
 memcpy(d->pixels,d->pause_presentation,480000);memcpy(d->palette,d->pause_palette,768);free(d->pause_presentation);d->pause_presentation=NULL;
 /* Keys pressed before the pause were released inside the menu, outside the
  * engine's input; a stale held Escape would re-enter the pause at once. */
 memset(d->held,0,sizeof(d->held));d->pending_count=0;d->left_held=0;
 return 0;
}
/* SAVE_SLOT from the in-game list 14: the host's modern save of the paused game
 * FIRST (its retained gameplay picture, no AffGame draw of mode16), then the
 * original final 0x42c570 (MenuInit 18 + HUD latch) through rl_shell_resume.
 * *saved receives the save result; play resumes even when the save failed. */
static int shell_save_slot(MacOSLiveDriver*d,const char*path,int32_t*saved,MacOSSaveJob**job){
 if(!d||!d->shell||!path||!saved||d->shell_request.kind!=RL_SHELL_SAVE_SLOT||!d->pause_presentation)return RL_INVALID_ARGUMENT;
 uint8_t*shown=malloc(480000+768);if(!shown)return save_result(d,RL_IO_ERROR,"Salvar pela lista original");
 memcpy(shown,d->pixels,480000);memcpy(shown+480000,d->palette,768);
 memcpy(d->pixels,d->pause_presentation,480000);memcpy(d->palette,d->pause_palette,768);
 *saved=job?snapshot_capture(d,path,0,job):snapshot_file(d,path,0);
 char error[512];snprintf(error,sizeof(error),"%s",d->save_error);
 int c=shell_resume(d);
 if(c){memcpy(d->pixels,shown,480000);memcpy(d->palette,shown+480000,768);if(job){macos_live_save_job_free(*job);*job=NULL;}}
 else if(*saved)snprintf(d->save_error,sizeof(d->save_error),"%s",error);
 free(shown);return c;
}
int32_t macos_live_shell_save_slot(MacOSLiveDriver*d,const char*path,int32_t*saved){return shell_save_slot(d,path,saved,NULL);}
/* Same boundary, but only the capture runs here: *job (when *saved is 0) is
 * committed later by macos_live_save_commit on any thread; play resumes at once. */
int32_t macos_live_shell_save_slot_begin(MacOSLiveDriver*d,const char*path,int32_t*saved,MacOSSaveJob**job){
 if(!job)return RL_INVALID_ARGUMENT;*job=NULL;return shell_save_slot(d,path,saved,job);
}
/* Read-only view of the open session's memory (exported copy; the session,
 * its generation and the pending request are unchanged). */
int32_t macos_live_shell_peek(MacOSLiveDriver*d,uint32_t address,uint32_t size,uint8_t*out){
 if(!d||!d->shell||!out||!size||size>4096||!d->rl_shell_export_world)return RL_INVALID_ARGUMENT;
 RlMundo world=0;int c=d->rl_shell_export_world(d->shell,d->shell_generation,&world);if(c)return save_result(d,c,"Leitura da tela original");
 uint32_t n=0;uint8_t*b=NULL;c=d->rl_projetar_tudo(world,NULL,0,&n);
 if(c==RL_BUFFER_TOO_SMALL){b=malloc(n);c=b?d->rl_projetar_tudo(world,b,n,&n):RL_IO_ERROR;}else if(!c)c=RL_SAVE_INVALID;
 for(uint32_t i=0;!c&&i<size;i++)c=campaign_byte(b,n,address+i,&out[i]);
 free(b);d->rl_liberar_mundo(world);return c?save_result(d,c,"Leitura da tela original"):0;
}
/* Mouse zone rectangle (left, top, right, bottom) of the current screen, from the
 * session's MouseZone table at 0x5099b4+16*zone (0x479cac). */
int32_t macos_live_shell_zone(MacOSLiveDriver*d,uint32_t zone,int32_t*rect){
 if(!rect||zone>255)return RL_INVALID_ARGUMENT;
 uint8_t raw[16];int c=macos_live_shell_peek(d,0x5099b4+16*zone,16,raw);if(c)return c;
 for(unsigned i=0;i<4;i++)rect[i]=(int32_t)((uint32_t)raw[4*i]|(uint32_t)raw[4*i+1]<<8|(uint32_t)raw[4*i+2]<<16|(uint32_t)raw[4*i+3]<<24);
 return 0;
}
/* Host presentation policy for the objective/briefing pages (modes 17 and 23):
 * the original turns a page only while the button is down and 0x508f7c % 10 == 0
 * (0x42caa9 -> 0x47c7ca). A press is stamped at the next multiple of 10, so each
 * click turns exactly one page; a release (or a move) that would land on a
 * multiple of 10 already used is stamped one tick later, so it never turns a
 * second page. Never decreases the clock; held repeat stays every 10 ticks. */
uint32_t macos_live_shell_click_clock(uint32_t mode,uint32_t clock,uint32_t press){
 if(mode!=RL_SHELL_BRIEFING&&mode!=RL_SHELL_OBJECTIVES)return clock;
 if(press)return clock%10?clock+(10-clock%10):clock;
 return clock%10?clock:clock+1;
}
int32_t macos_live_shell_request(MacOSLiveDriver*d){
 if(!d||!d->shell)return RL_INVALID_ARGUMENT;
 uint32_t kind=d->shell_request.kind,mode=d->shell_request.mode;
 if(kind==RL_SHELL_RESET_CAMPAIGN&&mode==RL_SHELL_BRIEFING){
  uint8_t path[300];uint32_t pn=0,tn=0;uint8_t*text=NULL;
  int c=briefing_file(d,1,path,&pn,&text,&tn);
  if(!c){
   RlShellFile files[1]={{{path,pn},{text,tn}}};
   c=d->rl_shell_campaign_reset(d->shell,d->shell_generation,files,text?1:0,&d->shell_generation);
   if(!c){d->new_campaign=1;c=shell_entered(d,RL_SHELL_BRIEFING);}
  }
  free(text);return c?save_result(d,c,"Novo jogo original"):0;
 }
 if(kind==RL_SHELL_RESET_CAMPAIGN&&mode==RL_SHELL_LOAD_MENU){
  SlotFiles*slots=malloc(sizeof(*slots));if(!slots)return save_result(d,RL_IO_ERROR,"Carregar jogo original");
  int c=slot_files(d,slots);
  if(!c)c=d->rl_shell_campaign_reset(d->shell,d->shell_generation,slots->files,slots->count,&d->shell_generation);
  free(slots);if(!c)c=shell_entered(d,RL_SHELL_LOAD_MENU);
  return c?save_result(d,c,"Carregar jogo original"):0;
 }

 if(kind==RL_SHELL_RESUME)return shell_resume(d);
 if(kind==RL_SHELL_RESET_CAMPAIGN){
  snprintf(d->save_error,sizeof(d->save_error),"Pedido original %u/%u (%s) ainda não disponível na tela preparada",kind,mode,"Novo jogo");
  return RL_UNSUPPORTED_PATH;
 }
 return RL_INVALID_ARGUMENT;
}
static void event_flc_folder(MacOSLiveDriver*d,char*out,size_t size);
/* Exact <CD root>\\Events\\CREDITS.FLC VFS entry: the packaged file's first 128 bytes.
 * *header_length stays 0 when the file is absent (the original's failed open). */
static int credits_file(MacOSLiveDriver*d,uint8_t*path,uint32_t*pn,uint8_t*header,uint32_t*header_length){
 uint8_t*world=NULL;uint32_t wn=0;int c=shell_world_bytes(d,&world,&wn);
 if(!c)c=shell_string(world,wn,0x509594,path,256,pn);free(world);if(c)return c;
 memcpy(path+*pn,"\\Events\\CREDITS.FLC",19);*pn+=19;*header_length=0;
 char folder[4096],name[4200];event_flc_folder(d,folder,sizeof(folder));snprintf(name,sizeof(name),"%s/CREDITS.FLC",folder);
 FILE*f=fopen(name,"rb");if(!f)return errno==ENOENT?0:RL_IO_ERROR;
 size_t n=fread(header,1,128,f);fclose(f);if(n!=128)return RL_IO_ERROR;*header_length=128;return 0;
}
/* One AffCredits frame. ENDED (the FLC did not open) leaves SHOW main pending in
 * the engine: completed here, so the host only repaints. */
int32_t macos_live_shell_credits_frame(MacOSLiveDriver*d,uint32_t elapsed_ms,RlShellCredits*out){
 if(!d||!d->shell||!out||d->shell_mode!=RL_SHELL_CREDITS_SCREEN||d->shell_request.kind)return RL_INVALID_ARGUMENT;
 int c=d->rl_shell_credits_frame(d->shell,d->shell_generation,elapsed_ms,out);
 if(c)return save_result(d,c,"Créditos originais");
 d->shell_generation=out->generation;
 if(out->state==RL_SHELL_CREDITS_ENDED){
  c=d->rl_shell_menu_init(d->shell,d->shell_generation,NULL,0,&d->shell_generation);
  if(!c)c=shell_entered(d,RL_SHELL_MAIN);
  if(c)return save_result(d,c,"Retorno original dos créditos");
 }
 return 0;
}
/* Host's decoded FLC picture over the persistent picture, with the FLC palette. */
int32_t macos_live_shell_credits_present(MacOSLiveDriver*d,const RlShellCredits*frame,const uint8_t*pixels,const uint8_t*palette){
 if(!d||!frame||!pixels||!palette||d->shell_mode!=RL_SHELL_CREDITS_SCREEN||frame->state!=RL_SHELL_CREDITS_DRAW||frame->x+frame->width>800||frame->y+frame->height>600)return RL_INVALID_ARGUMENT;
 for(uint32_t y=0;y<frame->height;y++)memcpy(d->pixels+(frame->y+y)*800+frame->x,pixels+y*frame->width,frame->width);
 memcpy(d->palette,palette,768);return 0;
}
/* SHOW, the pause menu's SAVE_LOAD request (into lists 12/14) and CREDITS (mode10). */
int32_t macos_live_shell_show(MacOSLiveDriver*d){
 if(!d||!d->shell||(d->shell_request.kind!=RL_SHELL_SHOW&&d->shell_request.kind!=RL_SHELL_SAVE_LOAD&&d->shell_request.kind!=RL_SHELL_CREDITS))return RL_INVALID_ARGUMENT;
 uint32_t mode=d->shell_request.kind==RL_SHELL_CREDITS?RL_SHELL_CREDITS_SCREEN:d->shell_request.mode;
 uint8_t path[300],header[128];uint32_t pn=0,tn=0;uint8_t*text=NULL;unsigned number=0;int c=0;
 SlotFiles*slots=NULL;RlShellFile files[1];const RlShellFile*vfs=files;uint32_t count=0;
 if(is_list(mode)){slots=malloc(sizeof(*slots));c=slots?slot_files(d,slots):RL_IO_ERROR;if(!c){vfs=slots->files;count=slots->count;}}
 else if(mode==RL_SHELL_BRIEFING||mode==RL_SHELL_OBJECTIVES){c=stage_number(d,&number);if(!c)c=briefing_file(d,number,path,&pn,&text,&tn);files[0]=(RlShellFile){{path,pn},{text,tn}};count=text?1:0;}
 else if(mode==RL_SHELL_CREDITS_SCREEN){c=credits_file(d,path,&pn,header,&tn);files[0]=(RlShellFile){{path,pn},{header,tn}};count=tn?1:0;if(c)snprintf(d->save_error,sizeof(d->save_error),"Falha ao ler o CREDITS.FLC original");}
 uint32_t from=d->shell_mode;
 if(!c)c=d->rl_shell_menu_init(d->shell,d->shell_generation,vfs,count,&d->shell_generation);
 if(!c)c=shell_entered(d,mode);
 /* Pause "Reiniciar a missão": the briefing's START_MISSION reloads this stage. */
 if(!c&&mode==RL_SHELL_BRIEFING&&from==RL_SHELL_PAUSE)d->new_campaign=(int)number;
 free(text);free(slots);return c?save_result(d,c,"Tela original"):0;
}
