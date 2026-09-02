import {
  APP_ID,
  createPlatformFactory,
  DOCUMENT,
  ErrorHandler,
  platformCore,
  provideZonelessChangeDetection,
  RendererFactory2,
  ɵINJECTOR_SCOPE as INJECTOR_SCOPE,
  ɵinternalCreateApplication as internalCreateApplication,
  ɵsetDocument as setDocument,
  type ApplicationRef,
  type EnvironmentProviders,
  type PlatformRef,
  type Provider,
  type Type
} from '@angular/core'

import { createFakeDocument } from './document'
import { hotRefresh, rememberApplication, runningApplication } from './hot-refresh'
import { dom, NativeNode } from './native-node'
import { NativeRendererFactory } from './renderer'

/**
 * The native tree's root. It is created once per app: it is the `<an-view>`
 * everything hangs off, and the one the core mounts onto the view the platform
 * hands it.
 */
function createRootNode(): NativeNode {
  const root = new NativeNode('View', true)
  root.id = dom.createNode('View')
  dom.setStyle(root.id, 'width', '100%')
  dom.setStyle(root.id, 'height', '100%')
  dom.setRoot(root.id)
  return root
}

/**
 * A platform of our own. It does not inherit from `platform-browser`: that
 * package drags in the `DomAdapter`, the `DomRendererFactory2` and the HTML
 * sanitiser, all useless here and all assuming a DOM exists.
 */
export const platformNative: (extraProviders?: Provider[]) => PlatformRef =
  createPlatformFactory(platformCore, 'native', [
    { provide: DOCUMENT, useFactory: createFakeDocument, deps: [] }
  ]) as (extraProviders?: Provider[]) => PlatformRef

export interface NativeApplicationConfig {
  providers?: Array<Provider | EnvironmentProviders>
}

/**
 * The equivalent of `bootstrapApplication` for this platform.
 *
 * Zoneless is compulsory, not an option: without zone.js there is no patching of
 * timers or XHR inside the JS engine, and change detection is driven by signals,
 * which is exactly what the per-frame loop needs.
 */
export async function bootstrapNativeApplication(
  rootComponent: Type<unknown>,
  config: NativeApplicationConfig = {}
): Promise<ApplicationRef> {
  const already = runningApplication()
  if (already !== null) {
    // A second evaluation of the bundle: this is not a startup, it is a save.
    // Instead of mounting another app on top, the one already there is handed
    // the new definitions. Whoever asked for the reload looks at the flag to
    // know whether a real restart was needed.
    const globals = globalThis as typeof globalThis & { __anHotOk?: boolean }
    try {
      globals.__anHotOk = hotRefresh(rootComponent)
    } catch (error) {
      console.warn('[angular-native] el refresco en caliente falló, se reinicia:', error)
      globals.__anHotOk = false
    }
    return already
  }

  const document = createFakeDocument()
  setDocument(document)
  const root = createRootNode()

  const app = await internalCreateApplication({
    rootComponent,
    appProviders: [
      provideZonelessChangeDetection(),
      // Marks this injector as the root. Without it, no `providedIn: 'root'`
      // service resolves and the bootstrap dies with NG0201 on the first
      // internal token Angular asks for.
      { provide: INJECTOR_SCOPE, useValue: 'root' },
      // `platform-browser` normally provides it; here it has to go in by hand
      // or Angular aborts the bootstrap with NG0402.
      { provide: ErrorHandler, useClass: ErrorHandler },
      // It is for keeping styles and state apart between apps on the same page.
      // Here there is one app and no page, but the token is compulsory.
      { provide: APP_ID, useValue: 'an' },
      { provide: DOCUMENT, useValue: document },
      { provide: RendererFactory2, useFactory: () => new NativeRendererFactory(root), deps: [] },
      ...(config.providers ?? [])
    ]
  })
  rememberApplication(rootComponent, app)
  return app
}
