/**
 * The style names the core knows how to resolve.
 *
 * This duplicates the list that lives in `crates/an-layout/src/style.rs`, and
 * there is no way round it: the core needs it to resolve layout and this side
 * needs it to warn before sending something nobody is going to look at. That
 * they do not drift apart is checked by `scripts/check-styles.sh`, which
 * compares them.
 *
 * Without this warning, writing a name nobody recognises did nothing at all: the
 * property travelled to the host like any other prop, the host did not use it,
 * and that was the end of it. No error, no trace, nothing. It has cost three
 * afternoons across four separate cases.
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
