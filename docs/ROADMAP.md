# Frentes de trabalho / Work fronts

[Português](#português) · [English](#english)

Estado em 2026-11-12. / State on 2026-11-12.

| Frente / Front | Estado / State | Issue | Ajuda de fora / Outside help |
|---|---|---|---|
| Cliente Windows: jogo, menus, saves, Opções / Windows client: game, menus, saves, Options | Feito / Done | [#1](https://github.com/marciobarbato/rising-lands-public/issues/1) | — |
| Windows: builds MSVC x64 e ARM64 / MSVC x64 and ARM64 builds | Feito; sem assinatura de código, por decisão / Done; no code signing, by decision | — | — |
| Windows: teste num PC de verdade / test on a real PC | Fica com a comunidade / Left to the community | [#6](https://github.com/marciobarbato/rising-lands-public/issues/6) | **Bem-vinda / Welcome** |
| Windows: áudio, música do CD, FLC, filmes / audio, CD music, FLC, movies | Aberto; contribuição externa em discussão / Open; outside contribution under discussion | [#4](https://github.com/marciobarbato/rising-lands-public/issues/4) | Combinar na issue antes / Agree on the issue first |
| Host Windows: atalhos modernos, configurar teclas, caminhos melhores / Windows host: modern shortcuts, key setup, better paths | Feito; na pré-alfa 3 / Done; in pre-alpha 3 | [#7](https://github.com/marciobarbato/rising-lands-public/issues/7) (fechada / closed) | — |
| Atalhos modernos e fila de ordens / Modern shortcuts and order queue | Feito (macOS e Windows) / Done (macOS and Windows) | — | — |
| Tela de Opções no estilo original / Options screen in the original style | Feito (macOS e Windows) / Done (macOS and Windows) | — | — |
| Caminhos melhores (opcional) / Better unit paths (opt-in) | Feito no motor, no macOS e no Windows / Done in the engine, on macOS and on Windows | — | — |
| Outros CDs e idiomas / Other CDs and languages | Pesquisa / Research | [#8](https://github.com/marciobarbato/rising-lands-public/issues/8) | **Bem-vinda, de quem tem outras versões / Welcome, from owners of other versions** |
| Testes em jogo / In-game checks | Aberto / Open | [#9](https://github.com/marciobarbato/rising-lands-public/issues/9) | **Bem-vinda, de testadores / Welcome, from testers** |

**Regra para todas as frentes / Rule for every front:** nada derivado do jogo entra no repositório: nem dados do CD, nem
assets, nem textos traduzidos do jogo. Tudo vem do CD original do jogador. / Nothing derived from the game goes into the
repository: no CD data, no assets, no translated game text. Everything comes from the player's original CD.

Antes de começar algo grande, comente na issue da frente, para ninguém fazer o mesmo trabalho duas vezes. / Before
starting anything large, comment on the front's issue, so nobody does the same work twice.

---

## Português

### Cliente Windows

O cliente está em [`windows/`](../windows/README.md): Win32 + GDI em C, com o motor ligado estaticamente, num único
`RisingLands.exe`. O guia [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md) mostra o que cada parte faz e onde está no código do
macOS.

- **Feito:** importação do CD, os menus originais, a campanha, saves e cargas no mesmo formato do macOS (a pasta
  `Partidas` serve nos dois) e a tela de Opções. Com a mesma entrada, o driver do Windows e o do macOS produzem os
  mesmos quadros, o mesmo passeio pelos menus e os mesmos arquivos de save. Compila com mingw-w64 (x64) e com MSVC (x64
  e ARM64); o CI gera os três `.exe`, cada um com o seu SHA-256.
- **Sem assinatura de código:** decisão do projeto. O SmartScreen avisa na primeira execução; o SHA-256 e o commit de
  cada build estão no CI e nas notas da release.
- **Testar num PC com Windows real ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)):** o autor
  não vai testar no Windows, então isso fica com a comunidade. O CI só compila e roda testes sem o CD; ninguém jogou a
  build num PC com Windows ainda. Baixe a pré-alfa em
  [Releases](https://github.com/marciobarbato/rising-lands-public/releases) e conte na issue o que funcionou.
- **Áudio, música do CD, figuras de evento (FLC) e filmes ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)):**
  há uma contribuição externa em discussão. Combine na issue antes de começar.
- **Atalhos modernos, configurar teclas e caminhos melhores no host
  ([#7](https://github.com/marciobarbato/rising-lands-public/issues/7), fechada):** feitos, com os
  mesmos padrões do macOS; estão na pré-alfa 3. Detalhes em [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md).

### Atalhos modernos (estilo Age of Empires II)

Feito no macOS: N duas vezes centraliza no grupo, Home vai ao último alerta, ponto e vírgula escolhem o próximo
trabalhador ou militar ocioso, H seleciona o Santuário, e a **fila de ordens com Shift+clique** (pontos de passagem
marcados com bandeirinhas). Cada atalho vira uma entrada que o original já aceita; a fila é uma regra opcional do motor
(`RL_REGRA_FILA_ORDENS`), desligada no jogo original. No Windows os mesmos atalhos e a janela de configurar teclas
estão no código atual (`windows/src/rlw_modern.c`, `rlw_hotkeys.c`, `rlw_keys_dialog.c`); faltam lá as bandeirinhas da
fila sobre o mapa.

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
[#8](https://github.com/marciobarbato/rising-lands-public/issues/8) qual versão você tem. Nada traduzido do jogo pode
ir ao repositório.

### Testes em jogo

Partes que precisam de gente jogando ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)):

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
  the same menu walk and the same save files. It builds with mingw-w64 (x64) and with MSVC (x64 and ARM64); CI builds
  all three `.exe` files, each with its SHA-256.
- **No code signing:** a project decision. SmartScreen warns on the first run; each build's SHA-256 and commit are in
  CI and in the release notes.
- **Test on a real Windows PC ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)):** the author will
  not test on Windows, so this is left to the community. CI only builds and runs tests without the CD; nobody has played
  the build on a Windows PC yet. Download the pre-alpha from
  [Releases](https://github.com/marciobarbato/rising-lands-public/releases) and say on the issue what worked.
- **Audio, CD music, event pictures (FLC) and movies ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)):**
  an outside contribution is under discussion. Agree on the issue before starting.
- **Modern shortcuts, key dialog and better paths in the host
  ([#7](https://github.com/marciobarbato/rising-lands-public/issues/7), closed):** done, with the
  macOS defaults; they are in pre-alpha 3. Details in [`WINDOWS_CLIENT.md`](WINDOWS_CLIENT.md).

### Modern shortcuts (Age of Empires II style)

Done on macOS: N twice centres on the group, Home goes to the last alert, period and comma pick the next idle worker or
military unit, H selects the Sanctuary, and the **Shift+click order queue** (waypoints marked with small flags). Each
shortcut becomes input the original already accepts; the queue is an optional engine rule (`RL_REGRA_FILA_ORDENS`),
off in the original game. On Windows the same shortcuts and the key dialog are in the current source
(`windows/src/rlw_modern.c`, `rlw_hotkeys.c`, `rlw_keys_dialog.c`); the queue's flags over the map are still missing
there.

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
[#8](https://github.com/marciobarbato/rising-lands-public/issues/8) which version you have. No translated game text may
go into the repository.

### In-game checks

Parts that need people playing ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)):

- the **ESPELHO** (mirror) spell (30 mana);
- the **druid** and the **fusion monastery**;
- the **full campaign**, start to finish.

Help welcome from testers, on macOS or Windows.
