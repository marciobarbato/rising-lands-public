# Frentes de trabalho / Work fronts

[Português](#português) · [English](#english)

Estado em 2026-11-12. / State on 2026-11-12.

| Frente / Front | Estado / State | Issue | Ajuda de fora / Outside help |
|---|---|---|---|
| Cliente Windows: jogo, menus, saves / Windows client: game, menus, saves | Feito (marcos 1–3) / Done (milestones 1–3) | [#1](https://github.com/marciobarbato/rising-lands-public/issues/1), [#3](https://github.com/marciobarbato/rising-lands-public/issues/3) | Testar num PC com Windows / Test on a Windows PC |
| Windows: áudio, música do CD, FLC, filmes / audio, CD music, FLC, movies | Aberto; contribuição externa em discussão / Open; outside contribution under discussion | [#4](https://github.com/marciobarbato/rising-lands-public/issues/4) | Combinar na issue antes / Agree on the issue first |
| Windows: assinatura, MSVC, ARM64 / signing, MSVC, ARM64 | Aberto / Open | [#5](https://github.com/marciobarbato/rising-lands-public/issues/5) | Bem-vinda / Welcome |
| Atalhos modernos (estilo AoE2) / Modern shortcuts (AoE2 style) | Feito no macOS, com a fila de ordens / Done on macOS, with the order queue | [#1](https://github.com/marciobarbato/rising-lands-public/issues/1) | Levar ao Windows / Port to Windows |
| Caminhos melhores (opcional) / Better pathfinding (opt-in) | Em andamento pela equipe / In progress by the team | [#6](https://github.com/marciobarbato/rising-lands-public/issues/6) | Ainda não / Not yet |
| Tela de Opções no estilo original / Options screen in the original style | Planejada / Planned | [#7](https://github.com/marciobarbato/rising-lands-public/issues/7) | Ainda não / Not yet |
| Outros CDs e idiomas / Other CDs and languages | Pesquisa / Research | [#8](https://github.com/marciobarbato/rising-lands-public/issues/8) | Bem-vinda, de quem tem outras versões / Welcome, from owners of other versions |
| Testes em jogo / In-game checks | Aberto / Open | [#9](https://github.com/marciobarbato/rising-lands-public/issues/9) | Bem-vinda, de testadores / Welcome, from testers |

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

- **Feito (marcos 1–3):** importação do CD, os menus originais, a campanha, e saves e cargas no mesmo formato do macOS
  (a pasta `Partidas` serve nos dois). Com a mesma entrada, o driver do Windows e o do macOS produzem os mesmos quadros,
  o mesmo passeio pelos menus e os mesmos arquivos de save.
- **Testar num PC com Windows real:** ninguém jogou a build num PC com Windows ainda; o CI só compila e roda testes sem
  o CD. Baixe a pré-alfa em [Releases](https://github.com/marciobarbato/rising-lands-public/releases) e relate pelo
  modelo **Problema no jogo**, dizendo que é o Windows.
- **Áudio, música do CD, figuras de evento (FLC) e filmes ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)):**
  há uma contribuição externa em discussão. Combine na issue antes de começar.
- **Assinatura do código, builds MSVC e ARM64 ([#5](https://github.com/marciobarbato/rising-lands-public/issues/5)):**
  ajuda bem-vinda, principalmente de quem já assinou binários abertos para Windows.

### Atalhos modernos (estilo Age of Empires II)

Feito no macOS: N duas vezes centraliza no grupo, Home vai ao último alerta, ponto e vírgula escolhem o próximo
trabalhador ou militar ocioso, H seleciona o Santuário, e a **fila de ordens com Shift+clique** (pontos de passagem
marcados com bandeirinhas). Cada atalho vira uma entrada que o original já aceita; a fila é uma regra opcional do motor
(`RL_REGRA_FILA_ORDENS`), desligada no jogo original.

- **Levar ao Windows:** ajuda bem-vinda. O código de referência é `macos/src/NativeModernHotkeys.swift`; no Windows,
  ligar a fila é passar a regra em `RlMissaoInicio.regras`.

### Caminhos melhores (opcional, desligado por padrão)

Em andamento pela equipe, em validação. Será outra regra opcional do motor, desligada por padrão; sem ela o jogo
continua idêntico ao original. Por enquanto não é uma frente aberta a ajuda, para não dividir um trabalho que ainda
está mudando. Acompanhe na [#6](https://github.com/marciobarbato/rising-lands-public/issues/6).

### Tela de Opções no estilo original

Planejada: uma tela no visual do jogo para as opções que hoje ficam nos menus do aplicativo (atalhos modernos, regras
opcionais e outras). O desenho já existe; o trabalho ainda não começou.
[#7](https://github.com/marciobarbato/rising-lands-public/issues/7).

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

- **Done (milestones 1–3):** CD import, the original menus, the campaign, and saving and loading in the macOS format
  (the `Partidas` folder works on both). With the same input, the Windows and macOS drivers produce the same frames,
  the same menu walk and the same save files.
- **Test on a real Windows PC:** nobody has played the build on a Windows PC yet; CI only builds and runs tests without
  the CD. Download the pre-alpha from [Releases](https://github.com/marciobarbato/rising-lands-public/releases) and
  report with the **Problema no jogo / Game problem** template, saying it is Windows.
- **Audio, CD music, event pictures (FLC) and movies ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)):**
  an outside contribution is under discussion. Agree on the issue before starting.
- **Code signing, MSVC and ARM64 builds ([#5](https://github.com/marciobarbato/rising-lands-public/issues/5)):** help
  welcome, especially from people who have signed open-source Windows binaries.

### Modern shortcuts (Age of Empires II style)

Done on macOS: N twice centres on the group, Home goes to the last alert, period and comma pick the next idle worker or
military unit, H selects the Sanctuary, and the **Shift+click order queue** (waypoints marked with small flags). Each
shortcut becomes input the original already accepts; the queue is an optional engine rule (`RL_REGRA_FILA_ORDENS`),
off in the original game.

- **Port to Windows:** help welcome. The reference is `macos/src/NativeModernHotkeys.swift`; on Windows, turning on the
  queue means passing the rule in `RlMissaoInicio.regras`.

### Better pathfinding (opt-in, off by default)

In progress by the team, being validated. It will be another optional engine rule, off by default; without it the game
stays identical to the original. It is not open for outside help yet, so that work which is still changing is not
split. Follow it on [#6](https://github.com/marciobarbato/rising-lands-public/issues/6).

### Options screen in the original style

Planned: a screen in the game's look for the options that today live in the app menus (modern shortcuts, optional
rules and others). The design exists; work has not started.
[#7](https://github.com/marciobarbato/rising-lands-public/issues/7).

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
