# Rising Lands — motor reimplementado (macOS e Windows)

[English version](README.en.md)

Este repositório contém uma reimplementação do motor do jogo **Rising Lands** (1997, Microïds) em Rust e aplicativos nativos que rodam o jogo sobre esse motor: o do macOS (alfa) e o do Windows (pré-alfa, em andamento). A lógica foi portada rotina por rotina do executável original e comparada com ele.

**O jogo não vem junto.** Para jogar, você precisa da imagem do **CD original brasileiro** do Rising Lands. Na primeira execução, o aplicativo lê todos os dados do jogo desse CD: gráficos, paleta, sons, música, textos, fases e filmes. Nenhum dado do jogo é distribuído aqui, nem dentro do aplicativo.

## Baixar e jogar (macOS)

1. Baixe o `.dmg` da versão mais recente em **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**. A versão atual é a alfa `v0.1.0-alpha.2`. O aplicativo é assinado com Developer ID e notarizado pela Apple.
2. Abra o `.dmg` e arraste o **Rising Lands** para **Aplicativos**.
3. Consiga a imagem do CD (veja "Onde conseguir o CD").
4. Abra o aplicativo. Na janela **"Rising Lands — importar o CD original"**, clique em **Escolher o CD…** e selecione o arquivo **RISING_LANDS_BR_.BIN**. Também servem o .CUE, um .ISO ou a pasta com os arquivos do CD, mas a música só vem do .BIN.
5. Os arquivos são conferidos e copiados para a pasta de Suporte de Aplicativos uma única vez (cerca de 630 MB). Depois disso, o jogo abre direto no menu original. Para importar de novo: **Jogo → Importar o CD novamente…**.

Requisitos para jogar: Mac com **Apple Silicon** e **macOS 13** ou mais recente.

## Windows (pré-alfa)

O cliente Windows está em andamento, no diretório [`windows/`](windows/README.md). Existe uma **pré-alfa** para quem quiser testar cedo. **O teste num PC com Windows de verdade fica com a comunidade** ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)): o autor não vai testar no Windows.

1. Baixe o `.exe` da pré-release mais recente do Windows (`v0.1.0-prealpha.…-windows`) em **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**: x64 para a maioria dos PCs, ARM64 para PCs com processador ARM. Confira o SHA-256 publicado nas notas da release. O `.exe` foi compilado pelo próprio GitHub Actions a partir deste código (workflow `windows-client`).
2. **Aviso do SmartScreen:** o `.exe` **não é assinado**, então o Windows mostra "O Windows protegeu o computador". Clique em **Mais informações** e depois em **Executar assim mesmo**. Não há plano de assinar o executável; se preferir, compile você mesmo (veja abaixo).
3. Na primeira execução, uma janela pede o CD: escolha o **RISING_LANDS_BR_.BIN** (ou o .CUE, ou um .ISO). Os arquivos são conferidos por SHA-256 e copiados para `%LOCALAPPDATA%\RisingLands-ptbr\Dados`. Depois o jogo abre no menu original.

Requisitos: Windows 10 ou 11 de 64 bits (x86-64 ou ARM64). É um único `.exe`, sem instalador e sem DLLs extras.

O que já funciona:
- importação do CD (.BIN/.CUE, .ISO ou a pasta do CD);
- os menus originais: "Um jogador", "Novo jogo", briefing e objetivos, pausa (continuar, objetivos, abandonar, reiniciar), tela de resultado e próxima fase, créditos e "Sair";
- **saves**: as listas originais de salvar e carregar e o menu "Partida" (**F5** salva rápido, **F9** carrega rápido), com o autosave do início da fase. O formato é o mesmo do macOS: a pasta `Partidas` serve nos dois. Os saves entram a partir da pré-alfa 2;
- a **tela de Opções** no estilo original, nos menus principal e de pausa: dificuldade, velocidade, **Atalhos modernos** (os atalhos de tecla e a fila de ordens com Shift+clique) e **Caminhos melhores**, guardadas entre uma partida e outra, com os mesmos padrões do macOS (atalhos modernos ligados, caminhos melhores desligados; os atalhos modernos e os caminhos melhores valem a partir da pré-alfa 3);
- os **atalhos modernos** do macOS: o mesmo número duas vezes centraliza o grupo, **Home** vai ao último alerta, **.** e **,** escolhem o próximo trabalhador ou militar ocioso e **H** seleciona o santuário (a partir da pré-alfa 3);
- a **janela de configurar teclas** (item "Configurar teclas" das Opções ou menu **Partida → Configurar teclas…**), com as mesmas regras do macOS: o jogo continua recebendo só as 23 teclas do original (a partir da pré-alfa 3);
- a campanha, com mouse e teclado iguais aos do macOS, rolagem pela borda, tela cheia (**F11**), pausa (**P**) e velocidades (**+** e **−**: 0,5x, 1x, 2x e 4x).
- Paridade com o macOS: com a mesma entrada, o driver do Windows e o do macOS produzem os mesmos quadros, tick a tick, e o mesmo passeio pelos menus e os mesmos arquivos de save.

O que ainda falta:
- **áudio** (efeitos e música do CD), as figuras de evento (FLC) durante o jogo e os **filmes** ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)).

Importante: a paridade foi verificada no macOS, compilando o driver do Windows contra o mesmo motor. No Windows de verdade, o CI testa só a compilação, os testes unitários, o autoteste do motor e a recusa de um arquivo que não é o CD. **Ainda ninguém jogou essa build num PC com Windows.** Quem tiver Windows e o CD pode ajudar muito: conte na [#6](https://github.com/marciobarbato/rising-lands-public/issues/6) o que funcionou e o que não funcionou, ou abra uma issue pelo modelo **Problema no jogo** e diga que é o Windows.

## Onde conseguir o CD

A imagem do CD original brasileiro pode ser encontrada no **Internet Archive**. Baixe os arquivos **RISING_LANDS_BR_.BIN** e **RISING_LANDS_BR_.CUE**.

## Estado atual

- A campanha roda nas 25 fases. Um bot automático jogou todas elas, cada partida com até 72.000 quadros, sem nenhuma parada do motor. As primeiras missões também foram jogadas à mão sem problemas.
- Ninguém terminou a campanha inteira ainda. É aí que os testes de vocês ajudam.
- Se o jogo chegar a um trecho ainda não portado, ele não trava em silêncio: mostra um **painel de parada** com o endereço da rotina do original. Esse texto é o que mais ajuda num relato.
- macOS: alfa `v0.1.0-alpha.2`. O código já tem novidades que entram na próxima versão: o relógio do mosteiro com o painel lateral escondido, o balão saindo do hangar, os atalhos modernos e a fila de ordens com Shift+clique, a **tela de Opções**, os **caminhos melhores**, o menu **Áudio** e sons que antes não tocavam (o som dos eventos, o andar dos veículos, o impacto das torres; silêncio na pausa e na tela de resultado) (veja "Controles").
- Windows: pré-alfa, em andamento; jogo, menus, saves, tela de Opções, atalhos modernos, configurar teclas e caminhos melhores feitos, com builds x64 e ARM64 (veja "Windows (pré-alfa)").
- Só a versão brasileira do jogo, em português, é suportada.

## Controles

- **Velocidade:** menu **Jogo → Velocidade**, com 0,5x, 1x, 2x e 4x (15, 30, 60 e 120 quadros por segundo). A velocidade normal (1x) é 30 quadros por segundo. Atalhos: **⌘]** mais rápido, **⌘[** mais devagar, **⌘P** pausa e continua.
- **Partidas salvas** (no código atual, entra na próxima versão do macOS): **Jogo → Gerenciar partidas salvas…** lista as partidas com nome, fase e data, para **renomear** (o nome aparece nas listas do jogo) ou **apagar** (vai para a Lixeira). Com a janela aberta o jogo fica pausado. **Jogo → Salvamento automático** (desligado por padrão): a cada 5, 10 ou 15 minutos de jogo, num slot próprio que nunca ocupa as suas posições; ele aparece em **Jogo → Carregar partida**.
- **Som:** menu **Áudio**, com silenciar e volume (100%, 75%, 50%, 25%) dos efeitos e da música, guardados e valendo na hora (no código atual, entra na próxima versão do macOS).
- **Tela cheia:** menu **Janela → Tela cheia** (**⌃⌘F**). Em tela cheia aparece só o jogo, sobre fundo preto.
- **Andar pelo mapa:** encoste o mouse na borda. Em tela cheia, basta levar o mouse até a borda da tela. Em janela, o mapa rola enquanto o mouse estiver dentro da janela, junto à borda. As setas do teclado também rolam o mapa.
- **Teclado:** Enter, Esc, Espaço, setas, Delete, X, Tab (minimapa) e os números 0–9, como no original. As teclas podem ser trocadas em **Jogo → Configurar teclas…**. A tecla F11 do original (trocar para 640x480) não é usada.
- **Atalhos modernos** (no estilo de Age of Empires II; no código atual, entram na próxima versão do macOS). Cada atalho vira uma entrada que o original já aceita (tecla ou clique), pelo mesmo caminho do jogador; nada mexe no estado do jogo por fora.
  - **N duas vezes** (grupos 1–9): seleciona o grupo e centraliza nele (o Shift+N do original). Ctrl+N, N e Shift+N continuam como no original.
  - **Home:** último alerta (o X do original).
  - **.** (ponto): próximo trabalhador ocioso. **,** (vírgula): próximo militar ocioso.
  - **H:** seleciona o Santuário.
  - **Shift+clique** no chão com unidades selecionadas: enfileira um ponto de passagem. As unidades andam pelos pontos em ordem, e uma ordem comum substitui a fila. Os pontos aparecem como bandeirinhas ligadas por uma linha tracejada. No original, esse clique não faz nada; a fila é uma regra opcional do motor que segue a opção **Atalhos modernos**. Mudar a opção vale para os jogos começados depois; o jogo em andamento e as fases seguintes mantêm as regras com que ele começou, e um save guarda as suas.
  - Vêm ligados. Para desligar, desmarque **Jogo → Atalhos modernos**. As teclas podem ser trocadas em **Jogo → Configurar teclas…**. Os atalhos de tecla não funcionam nos menus, no briefing nem com o jogo pausado.
- **Caminhos melhores** (opcional, desligado por padrão; no código atual, entra na próxima versão do macOS): menu **Jogo → Caminhos melhores**. As unidades planejam o caminho inteiro antes de andar, um grupo espalha os destinos em vez de brigar por uma célula, e uma unidade bloqueada espera e recalcula em vez de desistir. Combate, IA e velocidades não mudam. Vale a partir do início da próxima fase; um save guarda as regras da partida. Detalhes em [docs/MODERN_PATHS.md](docs/MODERN_PATHS.md).
- **Tela de Opções** (no código atual, entra na próxima versão do macOS): o item **OPÇÕES**, no chão de madeira dos menus principal e de pausa, ou o menu **Jogo → Opções…** (**⌘O**), abre uma tela no visual do jogo com atalhos modernos, caminhos melhores, dificuldade, velocidade e configurar teclas. Durante a partida, o **⌘O** primeiro pausa o jogo, como o Esc, e abre a tela por cima do menu de pausa. **Esc** ou **Voltar** fecha a tela. São as mesmas opções dos menus do aplicativo. O jogo original não tem essa tela; ela é desenhada só com o material do seu CD.

## Problemas

Abra uma *issue* neste repositório com o modelo **Problema no jogo**. Ajuda muito informar:
- a versão do macOS e o modelo do Mac;
- a fase e o que você estava fazendo;
- o texto inteiro do painel de parada, se ele apareceu.

Prints e vídeos da tela são bem-vindos. Não anexe saves nem arquivos do jogo: eles podem conter dados do jogo original.

## Como foi feito

A engenharia reversa foi feita com o **Ghidra**, junto com um emulador x86 que roda as rotinas originais lado a lado com o port. Boa parte do port foi feita com a ajuda do **Claude** (a IA da Anthropic): ler a desmontagem, escrever o código em Rust e montar testes diferenciais que comparam cada rotina portada com o executável original. Os comentários do código citam os nomes e endereços das rotinas originais.

## Compilar a partir do código

Requisitos: Mac com Apple Silicon e macOS 13+, **Xcode Command Line Tools** (`xcode-select --install`) e **Rust** estável (https://rustup.rs).

```bash
./build-macos.sh
```

O aplicativo fica em `build/RisingLands.app`. Para escolher outra pasta de saída: `./build-macos.sh <pasta>`. Por padrão a assinatura é *ad hoc*. Para assinar para distribuição, defina `RISING_SIGN_IDENTITY` com uma identidade Developer ID; `RISING_BUNDLE_ID` troca o identificador do aplicativo.

Testes:

```bash
cargo test --locked --manifest-path engine/Cargo.toml
```
```bash
./macos/tests/run.sh
```

Cliente Windows: a partir de um Mac ou Linux com mingw-w64, ou no Windows com MSYS2/Git Bash. No Windows também compila com o Visual Studio (MSVC), para x64 e ARM64: `windows\build-msvc.ps1 -Test` no PowerShell. Detalhes em [windows/README.md](windows/README.md).

```bash
rustup target add x86_64-pc-windows-gnu
```
```bash
./windows/build.sh test
```

O `.exe` fica em `windows/build/RisingLands.exe`.

Os testes que precisam dos arquivos originais do jogo pulam quando eles não estão presentes. Com `RL_CD_ROOT` apontando para a pasta do CD, `cd_derived_assets` confere que os dados gerados do CD (paleta, tabelas, telas do menu e cache de terreno) saem iguais aos do original, byte a byte. O que entra e o que fica fora deste repositório está em [PUBLICATION_AUDIT.md](PUBLICATION_AUDIT.md).

## Frentes de trabalho

O que está sendo feito, o estado de cada frente e onde alguém de fora pode ajudar: **[docs/ROADMAP.md](docs/ROADMAP.md)**. Feitos desde a última versão publicada do macOS: a tela de Opções e os caminhos melhores (no Windows, estão na pré-alfa 3). As frentes abertas têm uma issue: o cliente Windows ([#1](https://github.com/marciobarbato/rising-lands-public/issues/1)), áudio e filmes no Windows ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)), teste num PC com Windows ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)), outros CDs e idiomas ([#8](https://github.com/marciobarbato/rising-lands-public/issues/8)) e testes em jogo ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)). Antes de começar algo grande, comente na issue da frente.

## Ajuda bem-vinda

- **Testar o Windows num PC de verdade:** ninguém jogou a pré-alfa num PC com Windows ainda, e o autor não vai testar no Windows ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)).
- **Host Windows:** áudio, música do CD e filmes ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)); já há uma contribuição externa em discussão: combine na issue antes. O guia [docs/WINDOWS_CLIENT.md](docs/WINDOWS_CLIENT.md) mostra onde cada parte fica no código do macOS.
- **Testes em jogo:** a magia ESPELHO, o druida e o mosteiro de fusão, e a campanha completa ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)).
- **Outros idiomas:** hoje só o CD brasileiro funciona. Se você tem outro CD original (inglês, francês...), conte qual na [#8](https://github.com/marciobarbato/rising-lands-public/issues/8). Nada traduzido do jogo pode ir ao repositório.

## Licença

O código deste repositório é distribuído sob a licença MIT (veja [LICENSE](LICENSE)): qualquer pessoa pode ler, usar, modificar e redistribuir. A licença cobre só o código; não cobre o jogo original.

## Direitos

Rising Lands e todos os seus materiais originais (gráficos, sons, música, textos, fases, filmes e executável) continuam pertencendo aos seus titulares. Este repositório não distribui nenhum desses materiais e não concede licença sobre eles. O jogador usa a sua própria cópia do CD.
