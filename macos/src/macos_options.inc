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
int32_t macos_live_options_button(MacOSLiveDriver*d,int32_t rect[4]){
 if(!d||!rect||!d->options||!d->shell)return RL_INVALID_ARGUMENT;
 return d->rl_opcoes_botao(d->options,d->shell,rect);
}
