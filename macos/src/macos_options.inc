/* Port-only "Opções" screen consumer (rising_options.h). No game rules and no
 * stored settings here: Swift owns the preferences and applies the changes.
 * The engine draws the screen and the button from the CD's own material and
 * never touches the prepared shell session. */
int32_t macos_live_options_sync(MacOSLiveDriver*d,const RlOpcoesValores*values){
 if(!d||!values)return RL_INVALID_ARGUMENT;
 if(d->validation||!d->rl_opcoes_versao||!d->rl_opcoes_criar||!d->rl_opcoes_definir||!d->rl_opcoes_entrada||!d->rl_opcoes_sobrepor||!d->rl_opcoes_desenhar||!d->rl_opcoes_botao||!d->rl_opcoes_fechar||!d->rl_opcoes_liberar||d->rl_opcoes_versao()!=RL_OPCOES_VERSION)return RL_UNSUPPORTED_PATH;
 if(d->options)return d->rl_opcoes_definir(d->options,values);
 /* The original menu background, decoded from the imported CD file. */
 uint32_t n=0;uint8_t*f=file(d->root,"TITRE.PIE",&n),*p=malloc(480000);
 int c=f&&p?d->rl_shell_imagem_cd(f,n,p,480000):RL_IO_ERROR;free(f);
 if(!c)c=d->rl_opcoes_criar(values,(RlShellBytes){p,480000},&d->options);
 free(p);if(c){d->options=0;return c;}
 /* An open main/pause picture gets its button right away. */
 if(d->shell&&!d->shell_request.kind&&(d->shell_mode==RL_SHELL_MAIN||d->shell_mode==RL_SHELL_PAUSE))(void)shell_copy_picture(d);
 return 0;
}
uint32_t macos_live_options_open(MacOSLiveDriver*d){return d&&d->options&&d->options_open?1:0;}
int32_t macos_live_options_input(MacOSLiveDriver*d,const RlShellInput*input,uint32_t keys,RlOpcoesEvento*out){
 if(!d||!input||!out)return RL_INVALID_ARGUMENT;
 memset(out,0,sizeof(*out));out->item=UINT32_MAX;
 if(!d->options||!d->shell||d->shell_request.kind||d->validation)return 0;
 /* Shell ticks advance the counters by one; the screen animates only by them. */
 uint32_t ticks=d->options_open?input->animation_84-d->shell_input.animation_84:0;
 RlOpcoesEntrada e={RL_OPCOES_VERSION,keys&63u,input->mouse_x,input->mouse_y,input->mouse_ae,input->mouse_af,ticks>8?1:ticks,0};
 int c=d->rl_opcoes_entrada(d->options,d->shell,&e,out);
 if(c){memset(out,0,sizeof(*out));out->item=UINT32_MAX;return save_result(d,c,"Tela de opções");}
 d->options_open=out->open!=0;
 if(!out->consumed)return 0;
 if(out->open)c=d->rl_opcoes_desenhar(d->options,d->shell,d->pixels,480000,d->palette,768);
 else c=shell_copy_picture(d);
 return c?save_result(d,c,"Tela de opções"):0;
}
/* Menu/keyboard shortcut: the same screen the button opens, through the
 * explicit rl_opcoes_abrir. Only over main2/pause16 without a pending request;
 * the shell session is not touched. Already open: nothing to do. */
int32_t macos_live_options_show(MacOSLiveDriver*d){
 if(!d||d->validation||!d->options||!d->shell||!d->rl_opcoes_abrir)return RL_UNSUPPORTED_PATH;
 if(d->shell_request.kind||(d->shell_mode!=RL_SHELL_MAIN&&d->shell_mode!=RL_SHELL_PAUSE))return RL_INVALID_ARGUMENT;
 if(d->options_open)return 0;
 int c=d->rl_opcoes_abrir(d->options,d->shell);
 if(c)return save_result(d,c,"Tela de opções");
 d->options_open=1;
 c=d->rl_opcoes_desenhar(d->options,d->shell,d->pixels,480000,d->palette,768);
 return c?save_result(d,c,"Tela de opções"):0;
}
int32_t macos_live_options_button(MacOSLiveDriver*d,int32_t rect[4]){
 if(!d||!rect||!d->options||!d->shell)return RL_INVALID_ARGUMENT;
 return d->rl_opcoes_botao(d->options,d->shell,rect);
}
/* Port-only "Redistribuir descobertas" consumer (rising_research.h). The
 * engine owns every rule (limits, free points, giving them back when the
 * briefing is left inside rl_shell_input) and the drawing; the host stores the
 * Options value, routes the clicks and repaints. */
static int redistribution_available(MacOSLiveDriver*d){
 return !d->validation&&d->rl_redistribuir_versao&&d->rl_redistribuir_estado&&d->rl_redistribuir_alvo&&d->rl_redistribuir_passo&&d->rl_redistribuir_sobrepor&&d->rl_redistribuir_versao()==RL_REDISTRIBUIR_VERSION;
}
static int redistribution_briefing(MacOSLiveDriver*d){
 return d->shell&&!d->shell_request.kind&&d->shell_mode==RL_SHELL_BRIEFING&&!d->options_open;
}
static int redistribution_active(MacOSLiveDriver*d){
 return d&&d->redistribuir&&redistribution_available(d)&&redistribution_briefing(d);
}
int32_t macos_live_redistribution_enable(MacOSLiveDriver*d,uint32_t on,uint32_t*repainted){
 if(!d||!repainted||on>1)return RL_INVALID_ARGUMENT;
 *repainted=0;
 if(!redistribution_available(d)){d->redistribuir=0;return RL_UNSUPPORTED_PATH;}
 if(d->redistribuir==on)return 0;
 d->redistribuir=on;
 if(!redistribution_briefing(d))return 0;
 /* A fresh copy of the session's picture, with the overlay only when on. */
 int c=shell_copy_picture(d);if(!c)*repainted=1;
 return c?save_result(d,c,"Redistribuir descobertas"):0;
}
int32_t macos_live_redistribution_target(MacOSLiveDriver*d,uint32_t x,uint32_t y,uint32_t*ramo,int32_t*delta){
 if(!d||!ramo||!delta)return RL_INVALID_ARGUMENT;
 *ramo=0;*delta=0;
 if(!redistribution_active(d))return 0;
 uint32_t r=0;int32_t k=0;
 /* A failed hit test (it never changes the session) is no button: the click goes to the shell. */
 if(d->rl_redistribuir_alvo(d->shell,x,y,&r,&k)||k<-1||k>1||r>3)return 0;
 *ramo=r;*delta=k;return 0;
}
int32_t macos_live_redistribution_step(MacOSLiveDriver*d,uint32_t ramo,int32_t delta,uint32_t*changed){
 if(!d||!changed||ramo>3||(delta!=1&&delta!=-1))return RL_INVALID_ARGUMENT;
 *changed=0;
 if(!redistribution_active(d))return 0;
 uint64_t g=d->shell_generation;uint32_t m=0;
 int c=d->rl_redistribuir_passo(d->shell,d->shell_generation,ramo,delta,&g,&m);
 if(c)return save_result(d,c,"Redistribuir descobertas");
 if(!m)return 0;
 /* The step advanced the session like an input: keep its generation, then
  * repaint the briefing (background + research block) and copy it with the overlay. */
 d->shell_generation=g;*changed=1;
 c=shell_paint(d);
 return c?save_result(d,c,"Redistribuir descobertas"):0;
}
