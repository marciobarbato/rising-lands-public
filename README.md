# Rising Lands — motor reimplementado (macOS)

[English version](README.en.md)

Este repositório contém uma reimplementação do motor do jogo **Rising Lands** (1997, Microïds) em Rust e um aplicativo nativo para macOS que roda o jogo sobre esse motor. A lógica foi portada rotina por rotina do executável original e comparada com ele.

**O jogo não vem junto.** Para jogar, você precisa da imagem do **CD original brasileiro** do Rising Lands. Na primeira execução, o aplicativo lê todos os dados do jogo desse CD: gráficos, paleta, sons, música, textos, fases e filmes. Nenhum dado do jogo é distribuído aqui, nem dentro do aplicativo.

## Baixar e jogar

1. Baixe o `.dmg` da versão mais recente em **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**. A versão atual é a alfa `v0.1.0-alpha.2`. O aplicativo é assinado com Developer ID e notarizado pela Apple.
2. Abra o `.dmg` e arraste o **Rising Lands** para **Aplicativos**.
3. Consiga a imagem do CD (veja "Onde conseguir o CD").
4. Abra o aplicativo. Na janela **"Rising Lands — importar o CD original"**, clique em **Escolher o CD…** e selecione o arquivo **RISING_LANDS_BR_.BIN**. Também servem o .CUE, um .ISO ou a pasta com os arquivos do CD, mas a música só vem do .BIN.
5. Os arquivos são conferidos e copiados para a pasta de Suporte de Aplicativos uma única vez (cerca de 630 MB). Depois disso, o jogo abre direto no menu original. Para importar de novo: **Jogo → Importar o CD novamente…**.

Requisitos para jogar: Mac com **Apple Silicon** e **macOS 13** ou mais recente.

## Onde conseguir o CD

A imagem do CD original brasileiro pode ser encontrada no **Internet Archive**. Baixe os arquivos **RISING_LANDS_BR_.BIN** e **RISING_LANDS_BR_.CUE**.

## Estado atual (alfa)

- A campanha roda nas 25 fases. Um bot automático jogou todas elas, cada partida com até 72.000 quadros, sem nenhuma parada do motor. As primeiras missões também foram jogadas à mão sem problemas.
- Ninguém terminou a campanha inteira ainda. É aí que os testes de vocês ajudam.
- Se o jogo chegar a um trecho ainda não portado, ele não trava em silêncio: mostra um **painel de parada** com o endereço da rotina do original. Esse texto é o que mais ajuda num relato.
- Só existe a versão para macOS. O cliente para Windows ainda não existe (veja "Ajuda bem-vinda").
- Só a versão brasileira do jogo, em português, é suportada.

## Controles

- **Velocidade:** menu **Jogo → Velocidade**, com 0,5x, 1x, 2x e 4x (15, 30, 60 e 120 quadros por segundo). A velocidade normal (1x) é 30 quadros por segundo. Atalhos: **⌘]** mais rápido, **⌘[** mais devagar, **⌘P** pausa e continua.
- **Tela cheia:** menu **Janela → Tela cheia** (**⌘F**). Em tela cheia aparece só o jogo, sobre fundo preto.
- **Andar pelo mapa:** encoste o mouse na borda. Em tela cheia, basta levar o mouse até a borda da tela. Em janela, o mapa rola enquanto o mouse estiver dentro da janela, junto à borda. As setas do teclado também rolam o mapa.
- **Teclado:** Enter, Esc, Espaço, setas, Delete, X, Tab (minimapa) e os números 0–9, como no original. As teclas podem ser trocadas em **Jogo → Configurar teclas…**. A tecla F11 do original (trocar para 640x480) não é usada.

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

Os testes que precisam dos arquivos originais do jogo pulam quando eles não estão presentes. Com `RL_CD_ROOT` apontando para a pasta do CD, `cd_derived_assets` confere que os dados gerados do CD (paleta, tabelas, telas do menu e cache de terreno) saem iguais aos do original, byte a byte. O que entra e o que fica fora deste repositório está em [PUBLICATION_AUDIT.md](PUBLICATION_AUDIT.md).

## Ajuda bem-vinda

- **Cliente para Windows:** o motor tem uma interface em C (`engine/include`), e o guia [docs/WINDOWS_CLIENT.md](docs/WINDOWS_CLIENT.md) descreve os passos. Veja a issue #1.
- **Outros idiomas:** hoje só a versão brasileira é suportada. Outras versões, como o CD em inglês, exigiriam conferir o motor contra o executável e os arquivos daquela versão. Se você tem outra versão e quer ajudar, abra uma issue.

## Licença

O código deste repositório é distribuído sob a licença MIT (veja [LICENSE](LICENSE)): qualquer pessoa pode ler, usar, modificar e redistribuir. A licença cobre só o código; não cobre o jogo original.

## Direitos

Rising Lands e todos os seus materiais originais (gráficos, sons, música, textos, fases, filmes e executável) continuam pertencendo aos seus titulares. Este repositório não distribui nenhum desses materiais e não concede licença sobre eles. O jogador usa a sua própria cópia do CD.
