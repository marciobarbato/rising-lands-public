/* Jogo > Importar o CD novamente… (Win32): starts this executable again with
 * `--cd <source> --reimport` (see rlw_reimport.h). 0 when started; the caller
 * then closes everything that reads Dados and quits. */
#ifndef RLW_REIMPORT_WIN_H
#define RLW_REIMPORT_WIN_H
#include <windows.h>
int rlw_reimport_relaunch(const char *source);
#endif
