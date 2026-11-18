# Frentes de trabalho / Work fronts

[Português](#português) · [English](#english)

Estado em 2026-11-14. / State on 2026-11-14.

| Frente / Front | Estado / State | Issue | Ajuda de fora / Outside help |
|---|---|---|---|
| Cliente Windows: jogo, menus, saves, Opções / Windows client: game, menus, saves, Options | Feito; na beta 1 / Done; in beta 1 | [#1](https://github.com/marciobarbato/rising-lands-public/issues/1) (fechada / closed) | — |
| Windows: build MSVC x64 / MSVC x64 build | Feito; só x86-64 desde 2026-11-14 (as pré-alfas 1 a 3 tinham ARM64); sem assinatura de código, por decisão / Done; x86-64 only since 2026-11-14 (pre-alphas 1 to 3 had ARM64); no code signing, by decision | — | — |
| Beta: teste público (macOS e Windows; beta 6 atual) / Beta: public test (macOS and Windows; beta 6 current) | Aberto desde 2026-11-14 / Open since 2026-11-14 | [#10](https://github.com/marciobarbato/rising-lands-public/issues/10) | **Bem-vinda: jogar, Windows num PC de verdade, outros CDs / Welcome: play, Windows on a real PC, other CDs** |
| Windows: teste num PC de verdade / test on a real PC | Fica com a comunidade / Left to the community | [#10](https://github.com/marciobarbato/rising-lands-public/issues/10) (#6 fechada / closed) | **Bem-vinda / Welcome** |
| Windows: áudio, música do CD, FLC, filmes / audio, CD music, FLC, movies | Feito; na beta 1 / Done; in beta 1 | [#4](https://github.com/marciobarbato/rising-lands-public/issues/4) (fechada / closed) | Testar num PC / Test on a PC (#10) |
| Volume a cada quadro (PostVolume 0x45448a), som dos filmes no volume da música (Windows) / Per-frame volume (PostVolume 0x45448a), movie sound at the music level (Windows) | Feito; na beta 2 / Done; in beta 2 | — | — |
| Seta do sistema sobre a barra do HUD (ShowCursor 0x49a054), ícone de nível sem piscar em saves antigos / System arrow over the HUD bar (ShowCursor 0x49a054), level icon no longer blinking in old saves | Feito; na beta 3 / Done; in beta 3 | — | — |
| Rolar o mapa com WASD (opção na tela de Opções) / Scroll the map with WASD (Options screen) | Feito; na beta 4 / Done; in beta 4 | — | — |
| Redistribuir descobertas no briefing; clique no claque de cinema das descobertas / Redistribute discoveries on the briefing; discovery clapperboard click | Feito; na beta 5 / Done; in beta 5 | — | — |
| Paradas da IA em listas de grupo que dão a volta (iagWarAffecteEnnemy e outras três) / AI stops on group lists that loop back (iagWarAffecteEnnemy and three more) | Feito; na beta 6 / Done; in beta 6 | — | — |
| Windows: menu Jogo, gerenciador de partidas salvas, salvamento automático, bandeirinhas da fila, DPI, ícone original, reimportar o CD / Jogo menu, saved-games manager, periodic autosave, order queue flags, DPI, original icon, CD re-import | Feito; na beta 1 / Done; in beta 1 | [#1](https://github.com/marciobarbato/rising-lands-public/issues/1) (fechada / closed) | — |
| Host Windows: atalhos modernos, configurar teclas, caminhos melhores / Windows host: modern shortcuts, key setup, better paths | Feito; na pré-alfa 3 / Done; in pre-alpha 3 | [#7](https://github.com/marciobarbato/rising-lands-public/issues/7) (fechada / closed) | — |
| Atalhos modernos e fila de ordens / Modern shortcuts and order queue | Feito (macOS e Windows) / Done (macOS and Windows) | — | — |
| Tela de Opções no estilo original / Options screen in the original style | Feito (macOS e Windows) / Done (macOS and Windows) | — | — |
| Caminhos melhores (opcional) / Better unit paths (opt-in) | Feito no motor, no macOS e no Windows / Done in the engine, on macOS and on Windows | — | — |
| Outros CDs e idiomas / Other CDs and languages | Pesquisa / Research | [#10](https://github.com/marciobarbato/rising-lands-public/issues/10) (#8 fechada / closed) | **Bem-vinda, de quem tem outras versões / Welcome, from owners of other versions** |
| Testes em jogo / In-game checks | Aberto / Open | [#10](https://github.com/marciobarbato/rising-lands-public/issues/10) (#9 fechada / closed) | **Bem-vinda, de testadores / Welcome, from testers** |

**Regra para todas as frentes / Rule for every front:** nada derivado do jogo entra no repositório: nem dados do CD, nem
assets, nem textos traduzidos do jogo. Tudo vem do CD original do jogador. / Nothing derived from the game goes into the
repository: no CD data, no assets, no translated game text. Everything comes from the player's original CD.

Antes de começar algo grande, comente na issue da frente (ou na [#10](https://github.com/marciobarbato/rising-lands-public/issues/10)), para ninguém fazer o mesmo trabalho duas
vezes. / Before starting anything large, comment on the front's issue (or on [#10](https://github.com/marciobarbato/rising-lands-public/issues/10)), so nobody does the same
work twice.

---

## Português

### Cliente Windows

O cliente está em [`windows/`](../windows/README.md): Win32 + GDI em C, com o motor ligado estaticamente, num único
`RisingLands.exe`. O guia [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md) mostra o que cada parte faz e onde está no código do
macOS.

- **Feito:** importação do CD, os menus originais, a campanha, saves e cargas no mesmo formato do macOS (a pasta
  `Partidas` serve nos dois) e a tela de Opções. Com a mesma entrada, o driver do Windows e o do macOS produzem os
  mesmos quadros, o mesmo passeio pelos menus e os mesmos arquivos de save. Compila com mingw-w64 e com MSVC, só
  x86-64 desde 2026-11-14 (as pré-alfas 1 a 3 também tinham um `.exe` ARM64; o Windows em ARM pode rodar o x64 pela
  própria emulação, sem suporte); o CI gera os dois `.exe`, cada um com o seu SHA-256, e roda o `.exe` sem dados do
  jogo (recusas da importação, primeira execução, bandeirinhas, DPI, áudio).
- **Feito na pré-alfa 4, publicada como beta 1 (a versão atual, só x86-64):** efeitos sonoros e o menu Áudio, a música do CD (do
  `.BIN` do próprio jogador), as figuras de evento (FLC) e os filmes
  ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)); o menu Jogo do macOS, o gerenciador de
  partidas salvas, o salvamento automático periódico, a partida salva pela pausa voltando jogando, as bandeirinhas
  da fila de ordens sobre o mapa, a janela ciente de DPI, o ícone original (lido do `RISING.EXE` importado) com
  atalhos no menu Iniciar e na Área de Trabalho, e Jogo → Importar o CD novamente… (quem vem das pré-alfas 1 a 3
  precisa reimportar para ter sons e música).
- **Na beta 2:** o volume que o jogo ajusta a cada quadro (PlaySnd_PostVolume, 0x45448a: o som de
  um veículo em movimento acompanha a câmera e para quando sai do alcance, como no original), também no macOS, e o som
  dos filmes seguindo o volume e o silenciar da música do menu Áudio.
- **Na beta 3:** a seta do sistema aparece fora da área do mapa (a barra de baixo com os ícones de construção), como
  no original (AffGame chama ShowCursor e para de desenhar o próprio cursor), em vez de o cursor sumir; e o ícone de
  nível não pisca mais em campanhas começadas antes de 11/11. Também no macOS.
- **Na beta 4:** a opção ROLAR COM WASD na tela de Opções (desligada por padrão): W, A, S e D rolam o mapa como as
  setas do original, a menos que o jogador tenha dado outra função a essas teclas. Também no macOS.
- **Na beta 5:** a opção REDISTRIBUIR DESCOBERTAS (−/+ nos pilares de pesquisa do briefing, ligada por padrão) e o
  clique no claque de cinema de uma descoberta, que agora abre a animação. Também no macOS.
- **Na beta 6:** o jogo não para mais com "iagWarAffecteEnnemy invalid group list" quando a IA manda um grupo contra
  quem a atacou (relato de jogador), nem em outras três rotinas da IA que percorrem listas de grupo: uma lista que dá
  a volta só para o jogo onde o original travaria. Também no macOS.
- **Sem assinatura de código:** decisão do projeto. O SmartScreen avisa na primeira execução; o SHA-256 e o commit de
  cada build estão no CI e nas notas da release.
- **Testar num PC com Windows real ([#10](https://github.com/marciobarbato/rising-lands-public/issues/10)):** o autor
  não vai testar no Windows, então isso fica com a comunidade. O CI só compila e roda testes sem o CD; ninguém jogou a
  build num PC com Windows ainda. Baixe a beta 6 em
  [Releases](https://github.com/marciobarbato/rising-lands-public/releases) e conte na issue o que funcionou.
- **Atalhos modernos, configurar teclas e caminhos melhores no host
  ([#7](https://github.com/marciobarbato/rising-lands-public/issues/7), fechada):** feitos, com os
  mesmos padrões do macOS; estão na pré-alfa 3. Detalhes em [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md).

### Atalhos modernos (estilo Age of Empires II)

Feito no macOS: N duas vezes centraliza no grupo, Home vai ao último alerta, ponto e vírgula escolhem o próximo
trabalhador ou militar ocioso, H seleciona o Santuário, e a **fila de ordens com Shift+clique** (pontos de passagem
marcados com bandeirinhas). Cada atalho vira uma entrada que o original já aceita; a fila é uma regra opcional do motor
(`RL_REGRA_FILA_ORDENS`), desligada no jogo original. No Windows os mesmos atalhos e a janela de configurar teclas
estão no código atual (`windows/src/rlw_modern.c`, `rlw_hotkeys.c`, `rlw_keys_dialog.c`), e as bandeirinhas da fila
sobre o mapa também (`rlw_markers.c`; desde a pré-alfa 4).

### Tela de Opções no estilo original

Feito no macOS e no Windows: o item **OPÇÕES** nos menus principal e de pausa abre uma tela desenhada pelo motor com o
material do CD (fundo, animações e fontes dos menus originais), com atalhos modernos, caminhos melhores, dificuldade,
velocidade e configurar teclas. O jogo original não tem essa tela; sem ela, o shell original fica idêntico. Contrato:
[`engine/include/options-screen.md`](../engine/include/options-screen.md).

### Caminhos melhores (opcional, desligado por padrão)

Feito no motor e no macOS (menu **Jogo → Caminhos melhores** ou o item das Opções). As unidades planejam o caminho
inteiro antes de andar, um grupo espalha os destinos e uma unidade bloqueada espera e recalcula em vez de desistir.
Desligada, o jogo é o original byte a byte. Detalhes em [`MODERN_PATHS.md`](MODERN_PATHS.md). No Windows, pelo item
das Opções (no código atual).

### Outros CDs e idiomas

Pesquisa. Hoje só o CD brasileiro funciona. Outras versões (inglês, francês e outras) exigem conferir o motor contra o
executável e os arquivos daquela versão. Ajuda bem-vinda de quem tem esses CDs originais: comece contando na
[#10](https://github.com/marciobarbato/rising-lands-public/issues/10) qual versão você tem (só o nome dos arquivos e o tamanho). Nada traduzido do jogo pode
ir ao repositório.

### Testes em jogo

Partes que precisam de gente jogando ([#10](https://github.com/marciobarbato/rising-lands-public/issues/10)):

- a magia **ESPELHO** (30 de mana);
- o **druida** e o **mosteiro de fusão**;
- a **campanha completa**, do começo ao fim.

Ajuda bem-vinda de testadores, no macOS ou no Windows.

---

## English

### Windows client

The client is in [`windows/`](../windows/README.md): Win32 + GDI in C, with the engine linked statically, in a single
`RisingLands.exe`. The guide [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md) maps each part to the macOS code.

- **Done:** CD import, the original menus, the campaign, saving and loading in the macOS format (the `Partidas` folder
  works on both) and the Options screen. With the same input, the Windows and macOS drivers produce the same frames,
  the same menu walk and the same save files. It builds with mingw-w64 and with MSVC, x86-64 only since 2026-11-14
  (pre-alphas 1 to 3 also shipped an ARM64 `.exe`; Windows on ARM may run the x64 one under its own emulation,
  unsupported); CI builds both `.exe` files, each with its SHA-256, and runs the `.exe` without game data (importer
  refusals, first start, flags, DPI, audio).
- **Done in pre-alpha 4, released as beta 1 (the current release, x86-64 only):** sound effects and the Áudio menu, CD music (from the
  player's own `.BIN`), event pictures (FLC) and movies
  ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)); the macOS Jogo menu, the saved-games manager,
  the periodic autosave, a pause-list save loading back into play, the order queue flags over the map, the DPI-aware
  window, the original icon (read from the imported `RISING.EXE`) with Start menu and Desktop shortcuts, and Jogo →
  Importar o CD novamente… (players coming from pre-alphas 1 to 3 must re-import to get sound and music).
- **In beta 2:** the per-frame volume the game sets (PlaySnd_PostVolume, 0x45448a: a moving vehicle's
  sound follows the camera and stops out of range, as in the original), on macOS too, and movie sound following the
  Áudio menu's music level and mute.
- **In beta 3:** the system arrow shows outside the map view (the bottom bar with the building icons), as in the
  original (AffGame calls ShowCursor and stops drawing its own pointer), instead of the pointer vanishing; and the level
  icon no longer blinks in campaigns started before 2026-11-11. On macOS too.
- **In beta 4:** the ROLAR COM WASD option in the Options screen (off by default): W, A, S and D scroll the map like
  the original's arrows, unless the player gave those keys another job. On macOS too.
- **In beta 5:** the REDISTRIBUIR DESCOBERTAS option (−/+ on the briefing's research pillars, on by default) and the
  discovery clapperboard click, which now opens the animation. On macOS too.
- **In beta 6:** the game no longer stops with "iagWarAffecteEnnemy invalid group list" when the AI sends a group
  against whoever attacked it (player report), nor in three more AI routines that walk group lists: a list that loops
  back only stops the game where the original would freeze. On macOS too.
- **No code signing:** a project decision. SmartScreen warns on the first run; each build's SHA-256 and commit are in
  CI and in the release notes.
- **Test on a real Windows PC ([#10](https://github.com/marciobarbato/rising-lands-public/issues/10)):** the author will
  not test on Windows, so this is left to the community. CI only builds and runs tests without the CD; nobody has played
  the build on a Windows PC yet. Download beta 6 from
  [Releases](https://github.com/marciobarbato/rising-lands-public/releases) and say on the issue what worked.
- **Modern shortcuts, key dialog and better paths in the host
  ([#7](https://github.com/marciobarbato/rising-lands-public/issues/7), closed):** done, with the
  macOS defaults; they are in pre-alpha 3. Details in [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md).

### Modern shortcuts (Age of Empires II style)

Done on macOS: N twice centres on the group, Home goes to the last alert, period and comma pick the next idle worker or
military unit, H selects the Sanctuary, and the **Shift+click order queue** (waypoints marked with small flags). Each
shortcut becomes input the original already accepts; the queue is an optional engine rule (`RL_REGRA_FILA_ORDENS`),
off in the original game. On Windows the same shortcuts and the key dialog are in the current source
(`windows/src/rlw_modern.c`, `rlw_hotkeys.c`, `rlw_keys_dialog.c`), and so are the queue's flags over the map
(`rlw_markers.c`; since pre-alpha 4).

### Options screen in the original style

Done on macOS and Windows: the **OPÇÕES** item on the main and pause menus opens a screen drawn by the engine with
material from the CD (the original menus' background, animations and fonts), with modern shortcuts, better paths,
difficulty, speed and key setup. The original game has no such screen; without it, the original shell is unchanged.
Contract: [`engine/include/options-screen.md`](../engine/include/options-screen.md).

### Better unit paths (opt-in, off by default)

Done in the engine and on macOS (menu **Jogo → Caminhos melhores** or the Options item). Units plan the whole path
before walking, a group spreads its goals, and a blocked unit waits and replans instead of giving up. Off, the game is
the original byte for byte. Details in [`MODERN_PATHS.md`](MODERN_PATHS.md). On Windows, through the Options item (in
the current source).

### Other CDs and languages

Research. Only the Brazilian CD works today. Other versions (English, French and others) need the engine checked
against that version's executable and files. Help welcome from owners of those original CDs: start by saying on
[#10](https://github.com/marciobarbato/rising-lands-public/issues/10) which version you have (just the file names and sizes). No translated game text may
go into the repository.

### In-game checks

Parts that need people playing ([#10](https://github.com/marciobarbato/rising-lands-public/issues/10)):

- the **ESPELHO** (mirror) spell (30 mana);
- the **druid** and the **fusion monastery**;
- the **full campaign**, start to finish.

Help welcome from testers, on macOS or Windows.
