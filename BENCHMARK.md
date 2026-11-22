# Benchmark e pesquisa: como o Vislo se compara e o que vale melhorar

Pesquisa feita em 2026-10-29 em páginas públicas (READMEs, docs.rs, crates.io, MDN, developer.chrome.com). O que a fonte não
afirmava está marcado **não confirmado**; nada foi presumido.

## 1. Comparação com outros visualizadores

| Visualizador | Formatos | Metadados | Animação |
|---|---|---|---|
| **Vislo** | ~70 extensões (com #36) | EXIF agrupado, GPS, traduzido (#39) | WAAPI + CSS compositor |
| qView | depende dos plugins instalados (não confirmado) | não confirmado | GIF e similares |
| ImageGlass | "90+" | EXIF | GIF, WebP, APNG, SVG |
| nomacs | inclui AVIF, HEIF, JXL, EXR | histograma, EXIF de RAW | não documentado |
| JPEGView | JPEG, PNG, TIFF, PSD, WebP, JXL, HEIC, AVIF, TGA + RAW | não documentado | WebP, APNG, JXL animados |
| Oculante | ~45 | EXIF, histograma, cor do pixel | GIF |
| Honeyview / BandiView | 27 / inclui JXL, AVIF, EXR, SVG | EXIF + GPS | GIF, WebP, APNG |
| IrfanView / XnView MP | "500+" (XnView MP) | EXIF, IPTC, XMP | APNG, GIF, TIFF multipágina |

Fontes: as páginas públicas dos projetos (READMEs e sites de produto) e a página de formatos de cada um. Ristretto/gThumb:
as páginas não renderizaram, **não confirmado**. Só se registra o que cada produto anuncia (formatos, metadados,
animação); como cada um pré-carrega e decodifica por dentro não é copiado nem comparado aqui.

**Leitura.** Os visualizadores com mais formatos delegam a decodificadores externos. O Vislo cobre o uso comum (fotos,
RAW, PSD, vetoriais) e ainda não lê IPTC/XMP, que é onde XnView MP e IrfanView ficam à frente.

## 2. Fatos verificados que decidiram o escopo

| Tema | Fato | Decisão |
|---|---|---|
| JPEG XL | `jxl-oxide` 0.12.6, Rust puro, MIT/Apache, integração com `image` ≥ 0.25.8 | **adotado** (PR de formatos) |
| JPEG 2000 | `hayro-jpeg2000` 0.4.1, Rust puro, MIT/Apache, integração com `image`; mais lento que OpenJPEG | **adotado** com guarda de pixels |
| HEIC/HEIF | `heic` 0.1.6 é AGPL-3.0 ou comercial; `libheif-rs` 3.0.0 embute libheif (C, LGPL); via WIC depende da extensão HEVC da Store (afirmação só em fontes secundárias) | **adiado**: precisa de decisão de licença |
| RAW extra | ExifTool lista `arq`, `gpr`, `ori`, `k25` como RAW baseados em TIFF; `bay`, `drf`, `dcs`, `mdc`, `pxn`, `kc2`, `rwz` não confirmados | **adotados** os quatro confirmados |
| JPEG reduzido | `image` 0.25.10 usa `zune-jpeg` 0.5.x, que **não** tem decodificação reduzida (issue upstream #434, PR rascunho #498); `jpeg-decoder` 0.3.2 tem `Decoder::scale` (1/8, 1/4, 1/2, 1) mas está em modo manutenção | **adotado só para miniaturas** (seção 3) |
| CSS | `@starting-style` e `transition-behavior: allow-discrete` desde o Chrome 117; View Transitions congela a página durante o callback e só roda uma por vez | CSS puro para overlays; View Transitions **não** para navegação entre fotos |
| Versão do WebView2 | a página de notas não renderizou: **não confirmado** | checar em runtime se necessário |

## 3. Medição: miniaturas e redução nítida de JPEG de 24 MP

`cargo test --profile devrel bench_ -- --ignored --nocapture` (código em `src-tauri/src/bench.rs`). Duas fotos sintéticas de
6000 × 4000, JPEG q90: **noisy** (7,4 MB, grão de ISO alto) e **clean** (3,5 MB, ISO baixo). Mediana de 7 execuções,
caminho antigo (decodifica tudo, reduz uma cópia) × novo (decodificação reduzida + `fit_owned`), intercalados na mesma
execução. Máquina de desenvolvimento Windows 11; a variação entre rodadas é de ±15–20%, então leia como ordem de grandeza.

| Caso | Antes (ms) | Depois (ms) | Ganho |
|---|---|---|---|
| noisy · miniatura 192 | ≈ 297 | ≈ 233 | 1,3× |
| noisy · miniatura 512 | ≈ 304 | ≈ 205 | 1,5× |
| noisy · `/f` 1920×1080 | ≈ 377 | ≈ 372 | ≈ 1× |
| clean · miniatura 192 | ≈ 259 | ≈ 126 | **2,1×** |
| clean · miniatura 512 | ≈ 259 | ≈ 125 | **2,1×** |
| clean · `/f` 1920×1080 | ≈ 300 | ≈ 266 | 1,1× |

**O que a medição ensinou** (e por que o escopo ficou menor que o planejado):

1. A decodificação reduzida só compensa para encolhimentos de 4× ou mais. A 1/2 o `jpeg-decoder` não é mais rápido que o
   decodificador SIMD completo (`zune-jpeg`): medido 215 ms × 228 ms. Por isso a rota `/f` (tipicamente 2×–3×) não a usa.
2. A decodificação de Huffman domina em fotos ruidosas, então o ganho cai (1,3×–1,5×) e sobe para ~2× em fotos limpas, que
   são a maioria dos arquivos de celular.
3. Um custo escondido era a cópia de 72 MB (`to_rgb8()` em `resize::fit`, ~35 ms). `fit_owned` reaproveita o buffer;
   isso beneficia todas as rotas de redução, inclusive `/f` e GIF/PNG grandes convertidos.
4. A meta da issue (≥ 2× em miniatura de JPEG grande) foi atingida só em fotos limpas; em fotos ruidosas o ganho é ~1,4×.

## 4. Passos e transições: quanto tempo o olho espera

`npm run perf:motion` (frontend real contra o backend simulado de `vite.screens.config.ts`, Chromium headless,
1360 × 900, mediana de 5 passagens). O cenário "lento" modela um backend em que a foto inteira leva 600 ms e a prévia
embutida 150 ms — a proporção de um JPEG grande num disco frio.

| Medida | Antes | Depois |
|---|---|---|
| Passo para uma foto que ainda está decodificando (backend lento) | 235 ms | **4 ms** |
| Salto para uma foto sem prévia preparada (backend lento) | 233 ms | **158 ms** |
| Passo com a foto já decodificada | 1,6 ms | 1,4 ms |
| Estágio coberto durante a troca (mínimo por quadro) | 75 % | **100 %** |
| Fundo visível através da troca, 60 ms / 120 ms depois do passo | 21,2 % / 6,5 % | **0,8 % / 0,14 %** |
| Quadros lentos (> 20 ms) numa janela de 43 quadros | 0 | 0 |
| Pior quadro da janela | 5,8 ms | 5,7 ms |

**O que a medição ensinou:**

1. O tempo até o passo *começar* era dominado pela espera fixa de 70 ms antes de pedir a prévia, mais o próprio
   decodificador da prévia. Preparar a prévia do vizinho assim que a foto atual aparece e pedir a prévia em paralelo com
   essa espera tirou quase todo o atraso do caminho crítico.
2. O `stepMinCoverage` é lido do navegador durante a troca: com dois *fades* de mesma duração, a soma das opacidades
   cai a 75 % no meio, e é o fundo aparecendo. Segurar a foto que sai opaca até a que entra estar inteira mantém 100 %.
   `npm run perf:pixels` confirma nos pixels (fundo magenta atrás das fotos): 21 % → menos de 1 %.
3. O ritmo dos quadros (pior quadro, quadros lentos) não mudou: as animações já eram só `transform` e `opacity`, que o
   compositor roda fora da thread principal. O ganho aqui é de tempo e de constância, não de FPS.
4. No harness o início do app não muda de forma medível: em modo de desenvolvimento ele é dominado pelo carregamento dos
   módulos do Vite, não pelo código do Vislo.

`npm run perf:ui` (cenário fixo com a tira aberta, painéis e menu) ficou em 102 layouts e ~735 recálculos de estilo,
igual ao valor anterior: as mudanças na tira (`z-index` por alvo, sem laço de animação com a tira fechada) não aparecem
nesse cenário, mas também não custam nada.

## 5. Codificação JPEG e listagem da pasta (2026-11-16)

`cargo test --profile devrel --manifest-path src-tauri/Cargo.toml bench_encode bench_album -- --ignored --nocapture`.
Caminho antigo e novo intercalados na mesma execução, mediana de 7. Contêiner Linux de 4 núcleos (não a máquina Windows
das seções anteriores); duas rodadas, variação de ±10%.

| Caso | Antes (ms) | Depois (ms) | Ganho | Tamanho |
|---|---|---|---|---|
| 24 MP RGB → JPEG q92 (rota `/c`) | ≈ 660 | ≈ 345 | **1,9×** | 5,0 MB = 5,0 MB |
| 24 MP RGBA → JPEG q92 (alfa achatado) | ≈ 1190 | ≈ 440 | **2,7×** | 3,5 MB → 3,6 MB |
| 1620 × 1080 RGB → JPEG q90 (rota `/f`) | ≈ 46 | ≈ 24 | **1,9×** | 0,5 MB = 0,5 MB |
| Pasta com 5000 imagens + 1000 sidecars | ≈ 36,5 | ≈ 32 | 1,15× | — |

Com o novo codificador, a miniatura de 192 px de uma foto limpa de 24 MP caiu de ≈ 126 ms (seção 3) para ≈ 98–103 ms.

**O que mudou e o que a medição ensinou:**

1. O codificador JPEG do `image` era o maior custo depois da decodificação. O `jpeg-encoder` (Rust puro, SIMD AVX2
   detectado em tempo de execução) faz o mesmo trabalho na metade do tempo. Com a subamostragem padrão dele (4:2:0 abaixo
   de q90) o arquivo ficava 25% menor, mas perdia até 19 dB de PSNR; com cor em resolução cheia (4:4:4, como o anterior)
   PSNR e tamanho ficam iguais (q92: 51,5 dB × 51,8 dB), então é essa a configuração usada.
2. `to_jpeg` copiava a imagem inteira (`to_rgb8`) mesmo quando ela já era RGB 8 bits, o caso de toda miniatura e de toda
   rota `/f`. Agora codifica direto do buffer.
3. Achatar a transparência criava três buffers (fundo RGBA, sobreposição, cópia RGB); agora é uma passada que escreve
   direto o RGB. Isso explica o ganho maior no caso RGBA.
4. Na listagem, cada imagem fazia um `stat` pelo caminho completo depois de já ter a entrada do diretório. Agora usa os
   metadados da própria entrada: no Linux é um `stat` relativo ao diretório aberto (1,15×); no Windows a enumeração já
   traz tamanho e datas, então deixa de abrir um handle por arquivo. O ganho no Windows **não foi medido** aqui.
5. PNG foi medido e **não mudou**: o `image` já usa compressão rápida por padrão (≈ 300 ms para 24 MP RGBA); a
   compressão "Default" levaria 6–10 s.

## 6. HDR, rota `/c` e memória do cache (2026-11-16)

`cargo test --profile devrel --manifest-path src-tauri/Cargo.toml bench_hdr bench_convert -- --ignored --nocapture`
(mesmo contêiner Linux da seção 5, antigo × novo intercalados, mediana de 7). O decode no navegador foi medido com
`createImageBitmap` + desenho num canvas, no Chromium headless.

| Caso | Antes | Depois | Ganho |
|---|---|---|---|
| HDR/EXR 24 MP RGB float → 8 bits | ≈ 1230 ms | ≈ 230 ms | **5,3×** (diferença máx. de 1 nível) |
| HDR/EXR 24 MP RGBA float → 8 bits | ≈ 1160 ms | ≈ 280 ms | **4,1×** |
| `/c` cinza 24 MP (TIFF, DICOM, FITS…) | ≈ 650 ms | ≈ 160–200 ms | **3,5–4×** |
| `/c` RGBA opaco 24 MP (PSD, TIFF de editor): codificar | ≈ 300 ms (PNG) | ≈ 430 ms (JPEG) | 0,7× |
| idem, decodificar no navegador | ≈ 416 ms | ≈ 209 ms | 2× |
| idem, total e tamanho | ≈ 716 ms · 35,9 MB | ≈ 636 ms · 5,0 MB | 1,1× · **7× menor** |

**O que mudou e o que a medição ensinou:**

1. `to_display` fazia um `powf` por canal (72 milhões numa foto de 24 MP), uma cópia RGBA float inteira antes e uma
   cópia RGB no fim. Agora uma tabela de 16 385 entradas substitui o `powf` (teste: nunca mais de 1 nível de diferença
   da fórmula exata) e o buffer de 8 bits é escrito numa passada só.
2. Imagens em tons de cinza eram expandidas para RGB antes do JPEG. O codificador aceita um canal só: um terço dos dados.
3. O PSD sempre chega com canal alfa, mesmo opaco, então ia como PNG de 36 MB. Agora o alfa é verificado: se todo
   pixel é opaco, vai como JPEG q92. Codificar fica mais lento, mas o navegador decodifica na metade do tempo e a
   resposta é 7× menor; no total ganha pouco em tempo e muito em memória e cópia pelo protocolo. Transparência real
   continua em PNG.
4. O cache do frontend guardava sempre 7 fotos decodificadas (a atual, 4 à frente e 2 atrás). Com fotos de 100 MP isso é
   2,8 GB. Agora há um orçamento de 1,5 GiB (`src/cache-budget.ts`): pastas comuns não mudam (sete fotos de 50 MP
   cabem), e com 100 MP ficam a atual e as 3 mais próximas (1,6 GB). `npm run perf:motion` não mudou (mesmos valores
   com e sem a mudança neste contêiner).
5. Novo workflow `bench` roda todos os `bench_` no `windows-latest` e publica os números no resumo do job. Ele roda
   quando os benchmarks mudam ou por `workflow_dispatch`; é informativo e não bloqueia nada.

## 7. Galeria sem miniaturas e miniaturas de RAW (2026-11-19)

**Por que algumas imagens ficavam sem prévia.** Parte é esperada: um arquivo danificado, um RAW sem JPEG embutido ou um
HEIC num Windows sem as extensões HEIF/HEVC não têm decodificador que funcione. A maior parte era bug: a galeria usava
`loading="lazy"`, e o WebView pede de uma vez toda célula perto da tela. O pool do backend atende o pedido mais novo
primeiro e, com mais de 256 na fila, descarta o mais antigo com 503. As células na tela, pedidas primeiro, eram as
descartadas; o `onerror` então pedia a foto inteira (`/o` ou, para RAW/PSD/TIFF, `/c`, que roda o mesmo decodificador e
também podia ser descartado), e a célula ficava vazia. Agora:

1. a galeria só pede as células visíveis (`IntersectionObserver`), por uma fila com no máximo 8 pedidos em voo
   (`src/thumb.ts`); a célula que sai da tela antes da vez é esquecida sem custar decodificação;
2. AVIF e SVG, que o backend não decodifica, vão direto ao arquivo, sem a ida e volta que sempre falhava;
3. depois de uma falha, só formatos nativos tentam o arquivo; os outros mostram um padrão hachurado em vez de vazio;
4. respostas de erro saem com `Cache-Control: no-store` (antes herdavam `max-age=300`).

`npm run perf:gallery` (novo): frontend real no Chromium headless, pasta de 3000 fotos (1/5 RAW), e um modelo do pool
servido por interceptação de requisições (4 workers, mais novo primeiro, fila de 256, custo fixo: miniatura 60 ms,
conversão 400 ms), para que o limite de 6 conexões por host do navegador não esconda a fila, como no `vislo:`. Mediana
de 3 passagens; limite de 20 s por medida.

| Cenário | Medida | Antes | Depois |
|---|---|---|---|
| célula 200 px, 1360 × 900 | abrir: tela completa | ≈ 900 ms | **≈ 380 ms** |
| | salto de 1/3 da pasta | ≈ 1050 ms | **≈ 530 ms** |
| célula 96 px, 1360 × 900 | abrir | ≈ 3400 ms | **≈ 1460 ms** |
| | salto · descartados · pedidos de foto inteira | ≈ 7000 ms · 431 · 352 | **≈ 1520 ms · 0 · 0** |
| | rolagem rápida (5 telas em 1 s) | ≈ 5000 ms · 488 descartados | **≈ 1460 ms · 0** |
| célula 96 px, 2560 × 1440 | abrir: células vazias após 20 s | **293 de 312** | 0 (completa em ≈ 4,8 s) |
| | salto: células vazias após 20 s | **312 de 336** | 0 (≈ 5,2 s) |
| | miniaturas pedidas ao abrir | 600 (+583 fotos inteiras) | 312 |

**RAW.** A miniatura de um RAW decodificava inteira a prévia JPEG embutida, que câmeras atuais gravam em resolução
cheia. Agora ela passa pela mesma decodificação reduzida dos JPEGs (IDCT a 1/8, a ideia de redimensionar no domínio
DCT: Dugad e Ahuja, IEEE TCSVT 2001), com a mesma regra de orientação de `load`.
`cargo test --profile devrel --manifest-path src-tauri/Cargo.toml bench_raw -- --ignored --nocapture` (contêiner Linux
de 4 núcleos, prévia de 24 MP após um cabeçalho tipo RAF e 8 MB de dados de sensor, mediana de 7, duas rodadas):

| Caso | Antes (ms) | Depois (ms) | Ganho |
|---|---|---|---|
| RAW ruidoso · miniatura 192 | ≈ 264 | ≈ 130–139 | **1,9×** |
| RAW ruidoso · miniatura 512 | ≈ 227–260 | ≈ 126–143 | **1,8×** |
| RAW limpo · miniatura 192 | ≈ 185 | ≈ 70–82 | **2,4×** |
| RAW limpo · miniatura 512 | ≈ 158–182 | ≈ 75–86 | **2,1×** |

## 8. Avaliado e adiado

| Ideia | Motivo |
|---|---|
| Gestão de cor ICC → sRGB com `moxcms` (já no `image`) | exige fixture P3 e decisão sobre o WebView2 já gerenciar cor |
| Controles de animação (pausa, passo de quadro) | recurso, não desempenho |
| `turbojpeg` | C + cmake/nasm no build; ganho não medido |
| HEIC/HEIF | licença e custo de build (seção 2) |

## 9. Ordem natural da pasta (2026-11-21)

`cargo test --release --lib album_bench -- --ignored --nocapture` (Linux, 4 vCPU, alternando antes/depois): `open()` de uma
pasta com 50 000 arquivos caiu de **194–231 ms** para **106–115 ms**. Num microbenchmark à parte, com nomes de câmera
embaralhados (`IMG_…`, `DSC…`, `Photo N (copy M)`), só a ordenação foi de ≈ 250 ms para ≈ 50 ms (mediana de 7).

**O que a medição ensinou:** a comparação natural sem diferenciar maiúsculas convertia cada caractere para minúsculo em
toda comparação (≈ 800 mil numa pasta de 50 000). Agora cada nome é convertido uma vez, com a mesma regra por caractere, e
a ordem resultante é idêntica (teste `sort_natural_matches_case_insensitive_natord`, com acentos, `İ`, `Σ` e espaços).

## 10. EXIF de TIFF e RAW sem ler o arquivo inteiro (2026-11-22)

O leitor de EXIF carregava o arquivo inteiro sempre que ele era um TIFF (TIFF, DNG, NEF, CR2, ARW, PEF ...), só para
ler alguns KB de tags. Isso acontece no painel de informações, na rota da prévia embutida e na orientação de cada
decodificação. Agora ele tenta primeiro 1 MB do começo; se alguma tag ou IFD fica além disso, a leitura falha
("truncated") e cai para o arquivo inteiro, então o resultado é sempre o mesmo.

Arquivo de 72 MB com as tags no começo (o layout das câmeras), Linux 4 vCPU, mediana de 15:

| Caso | Antes | Depois |
|---|---|---|
| RAW, tags no começo | 37,4 ms | **0,08 ms** |
| TIFF do libtiff, IFD no fim | 36,7 ms | 37,6 ms (sem ganho, dentro do ruído) |

`bench_tiff_exif` reproduz o primeiro caso (o "antes" dele lê o arquivo e depois o copia, por isso fica em ~80 ms).
O teste `a_tiff_cut_anywhere_reads_the_same_tags_and_thumbnail_as_the_whole_file` corta o arquivo em cada byte possível
e exige as mesmas tags e a mesma miniatura do arquivo inteiro.
