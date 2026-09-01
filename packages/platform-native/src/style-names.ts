/**
 * Los nombres de estilo que el núcleo sabe resolver.
 *
 * Está aquí duplicada la lista que vive en `crates/an-layout/src/style.rs`, y
 * no se puede evitar: el núcleo la necesita para resolver el layout y este
 * lado la necesita para avisar antes de mandar algo que nadie va a mirar. Que
 * no se separen lo comprueba `scripts/check-styles.sh`, que las compara.
 *
 * Sin este aviso, escribir un nombre que nadie reconoce no hacía nada: la
 * propiedad viajaba al host como una prop cualquiera, el host no la usaba, y
 * ahí se acababa. Sin error, sin traza, sin nada. Ha costado tres tardes en
 * cuatro casos distintos.
 */
export const KNOWN_STYLES: ReadonlySet<string> = new Set([
  'alignContent',
  'alignItems',
  'alignSelf',
  'aspectRatio',
  'borderBottomWidth',
  'borderLeftWidth',
  'borderRightWidth',
  'borderTopWidth',
  'borderWidth',
  'bottom',
  'boxSizing',
  'columnGap',
  'display',
  'flex',
  'flexBasis',
  'flexDirection',
  'flexGrow',
  'flexShrink',
  'flexWrap',
  'gap',
  'height',
  'justifyContent',
  'left',
  'margin',
  'marginBottom',
  'marginEnd',
  'marginHorizontal',
  'marginLeft',
  'marginRight',
  'marginStart',
  'marginTop',
  'marginVertical',
  'maxHeight',
  'maxWidth',
  'minHeight',
  'minWidth',
  'overflow',
  'padding',
  'paddingBottom',
  'paddingEnd',
  'paddingHorizontal',
  'paddingLeft',
  'paddingRight',
  'paddingStart',
  'paddingTop',
  'paddingVertical',
  'position',
  'right',
  'rowGap',
  'top',
  'width'
])
