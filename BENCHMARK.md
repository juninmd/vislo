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

## 4. Avaliado e adiado

| Ideia | Motivo |
|---|---|
| Orçamento em bytes no cache do frontend | o ganho só aparece com fotos > 50 MP; precisa de medição própria |
| Gestão de cor ICC → sRGB com `moxcms` (já no `image`) | exige fixture P3 e decisão sobre o WebView2 já gerenciar cor |
| Controles de animação (pausa, passo de quadro) | recurso, não desempenho |
| `turbojpeg` | C + cmake/nasm no build; ganho não medido |
| HEIC/HEIF | licença e custo de build (seção 2) |
