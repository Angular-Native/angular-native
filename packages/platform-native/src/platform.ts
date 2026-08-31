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
import { dom, NativeNode } from './native-node'
import { NativeRendererFactory } from './renderer'

/**
 * Raíz del árbol nativo. Se crea una sola vez por app: es el `<View>` del que
 * cuelga todo y el que el core monta sobre la vista que da la plataforma.
 */
function createRootNode(): NativeNode {
  const root = new NativeNode(dom.createNode('View'), 'View')
  dom.setStyle(root.id, 'width', '100%')
  dom.setStyle(root.id, 'height', '100%')
  dom.setRoot(root.id)
  return root
}

/**
 * Plataforma propia. No se hereda de `platform-browser`: ese paquete arrastra
 * el `DomAdapter`, el `DomRendererFactory2` y el sanitizador de HTML, todo
 * inútil aquí y todo asumiendo que existe un DOM.
 */
export const platformNative: (extraProviders?: Provider[]) => PlatformRef =
  createPlatformFactory(platformCore, 'native', [
    { provide: DOCUMENT, useFactory: createFakeDocument, deps: [] }
  ]) as (extraProviders?: Provider[]) => PlatformRef

export interface NativeApplicationConfig {
  providers?: Array<Provider | EnvironmentProviders>
}

/**
 * Equivalente de `bootstrapApplication` para esta plataforma.
 *
 * Zoneless es obligatorio, no una opción: sin zone.js no hay que parchear
 * temporizadores ni XHR dentro del motor JS, y la detección de cambios la
 * disparan las señales, que es justo lo que el bucle por frame necesita.
 */
export async function bootstrapNativeApplication(
  rootComponent: Type<unknown>,
  config: NativeApplicationConfig = {}
): Promise<ApplicationRef> {
  const document = createFakeDocument()
  setDocument(document)
  const root = createRootNode()

  return internalCreateApplication({
    rootComponent,
    appProviders: [
      provideZonelessChangeDetection(),
      // Marca este inyector como la raíz. Sin esto, ningún servicio
      // `providedIn: 'root'` resuelve y el arranque muere con NG0201 en el
      // primer token interno que Angular pide.
      { provide: INJECTOR_SCOPE, useValue: 'root' },
      // Lo provee `platform-browser` normalmente; aquí hay que ponerlo a mano
      // o Angular aborta el arranque con NG0402.
      { provide: ErrorHandler, useClass: ErrorHandler },
      // Sirve para separar estilos y estado entre apps en la misma página.
      // Aquí solo hay una app y no hay página, pero el token es obligatorio.
      { provide: APP_ID, useValue: 'an' },
      { provide: DOCUMENT, useValue: document },
      { provide: RendererFactory2, useFactory: () => new NativeRendererFactory(root), deps: [] },
      ...(config.providers ?? [])
    ]
  })
}
