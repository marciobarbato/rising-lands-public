/* Jogo > Importar o CD novamente…: the relaunch of this executable (Win32). */
#include "rlw_reimport_win.h"
#include "rlw_reimport.h"
#include <stdio.h>
#include <stdlib.h>

int rlw_reimport_relaunch(const char *source) {
 wchar_t exe[MAX_PATH]; DWORD n = GetModuleFileNameW(NULL, exe, MAX_PATH);
 if (!n || n >= MAX_PATH || !source) return 1;
 char exe8[MAX_PATH * 4], line[MAX_PATH * 4 + 4200];
 if (!WideCharToMultiByte(CP_UTF8, 0, exe, -1, exe8, (int)sizeof(exe8), NULL, NULL)) return 1;
 if (rlw_reimport_command_line(exe8, source, line, sizeof(line))) return 1;
 int wn = MultiByteToWideChar(CP_UTF8, 0, line, -1, NULL, 0);
 wchar_t *wline = wn > 0 ? malloc((size_t)wn * sizeof(wchar_t)) : NULL;
 if (!wline) return 1;
 MultiByteToWideChar(CP_UTF8, 0, line, -1, wline, wn);
 STARTUPINFOW si; PROCESS_INFORMATION pi; ZeroMemory(&si, sizeof(si)); si.cb = sizeof(si); ZeroMemory(&pi, sizeof(pi));
 BOOL ok = CreateProcessW(exe, wline, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi);
 free(wline);
 if (!ok) return 1;
 CloseHandle(pi.hThread); CloseHandle(pi.hProcess);
 return 0;
}
