import { ɵɵreplaceMetadata as replaceMetadata, type ApplicationRef, type Type } from '@angular/core'

/**
 * Refresco en caliente: cambiar el código de los componentes sin tirar la app.
 *
 * Al guardar un fichero, el bundle nuevo se evalúa encima del que ya corre. Su
 * mitad de arriba —Angular, el framework— no se vuelve a evaluar, así que sigue
 * habiendo una sola copia de todo; la de abajo sí, y trae clases nuevas para
 * los mismos componentes.
 *
 * Aquí se emparejan las viejas con las nuevas y se le pasa a Angular la
 * definición nueva sobre la clase vieja. Angular rehace las vistas pero
 * conserva la instancia del componente, así que las señales y los campos siguen
 * donde estaban: se ve el cambio sin perder en qué pantalla estabas ni lo que
 * llevabas escrito.
 *
 * Lo que no cubre: cambios en la mitad de arriba —el framework, o una
 * dependencia—, y añadir o quitar componentes. En esos casos devuelve `false` y
 * quien llama recarga entero, que es lo único honesto: cualquier apaño dejaría
 * la pantalla enseñando código viejo sin decirlo.
 */

/** Lo que Angular cuelga de la clase. No es API pública, pero es estable. */
interface Def {
  debugInfo?: { className: string; filePath: string } | null
  dependencies?: unknown
  directiveDefs?: unknown
  pipeDefs?: unknown
}

/** Un componente, directiva o pipe compilado. */
interface Declaration extends Type<unknown> {
  ɵcmp?: Def
  ɵdir?: Def
  ɵpipe?: Def
  ɵfac?: unknown
}

/** La app que está corriendo, para poder compararla con la que llega. */
let running: { root: Declaration; app: ApplicationRef } | null = null

export function rememberApplication(root: Type<unknown>, app: ApplicationRef): void {
  running = { root: root as Declaration, app }
}

export function runningApplication(): ApplicationRef | null {
  return running?.app ?? null
}

function defOf(type: Declaration): Def | null {
  return type.ɵcmp ?? type.ɵdir ?? type.ɵpipe ?? null
}

/**
 * De dónde salió la clase. `ɵsetClassDebugInfo` lo emite el compilador en las
 * builds de desarrollo, y es lo único que sobrevive a recompilar: el nombre de
 * la clase solo no vale —dos ficheros pueden llamar igual a lo suyo—, y la
 * identidad del objeto es justo lo que acaba de cambiar.
 */
function keyOf(type: Declaration): string | null {
  const info = defOf(type)?.debugInfo
  return info ? `${info.filePath}@${info.className}` : null
}

function dependenciesOf(def: Def): Declaration[] {
  const raw = typeof def.dependencies === 'function' ? def.dependencies() : def.dependencies
  if (!Array.isArray(raw)) {
    return []
  }
  // Una dependencia puede venir como clase o envuelta en un objeto, que es
  // como se declaran las directivas de host.
  return raw
    .map((entry) => (typeof entry === 'object' && entry !== null ? Reflect.get(entry, 'directive') : entry))
    .filter((entry): entry is Declaration => typeof entry === 'function')
}

/** Todo lo que cuelga del componente raíz, indexado por su fichero y clase. */
function collect(root: Declaration): Map<string, Declaration> {
  const found = new Map<string, Declaration>()
  const seen = new Set<Declaration>()
  const pending: Declaration[] = [root]
  while (pending.length > 0) {
    const type = pending.pop()!
    if (seen.has(type)) {
      continue
    }
    seen.add(type)
    const def = defOf(type)
    if (!def) {
      continue
    }
    const key = keyOf(type)
    if (key !== null) {
      found.set(key, type)
    }
    pending.push(...dependenciesOf(def))
  }
  return found
}

/**
 * Hace que la definición nueva apunte a las clases viejas.
 *
 * Es el paso que conserva el estado. Las instancias que hay montadas son de las
 * clases viejas; si la plantilla nueva de un padre mandara construir las clases
 * nuevas de sus hijos, cada hijo nacería otra vez y perdería lo suyo.
 */
function rewire(type: Declaration, old: Map<string, Declaration>): void {
  const def = defOf(type)
  if (!def || def.dependencies == null) {
    return
  }
  const resolved = dependenciesOf(def).map((dep) => {
    const key = keyOf(dep)
    return (key !== null && old.get(key)) || dep
  })
  def.dependencies = () => resolved
}

/**
 * La lista de directivas —o de pipes— que puede usar una plantilla.
 *
 * Angular la resuelve una sola vez, al definir el componente, y la función que
 * deja guardada cierra sobre las dependencias de aquel momento: cambiar
 * `def.dependencies` después no la toca. Y al recargar en caliente,
 * `mergeWithExistingDefinition` conserva a propósito la lista que tuviera la
 * definición vieja, así que tampoco llega la del bundle nuevo.
 *
 * Con las dos puertas cerradas, la única forma de que la plantilla nueva
 * encuentre sus directivas es rehacer la lista aquí, a partir de las clases ya
 * reapuntadas. Se devuelve una función y no un array porque las definiciones
 * de esas clases se están cambiando en este mismo barrido: leerlas ahora daría
 * las viejas.
 *
 * Dejarla en `null` —que es lo que se hacía— no la hace recalcularse: la deja
 * vacía para siempre. Y no se nota casi: sin directivas, `[backgroundColor]`
 * deja de ser la entrada de `an-view` y pasa por `Renderer2.setProperty`, que
 * aquí acaba en el mismo `setProp`, así que la pantalla sigue pintando igual.
 * Lo que no sobrevive es lo que una directiva hace y una prop no —enganchar un
 * oyente, escribir estilos desde el host—, y eso se vio por primera vez en el
 * reloj: `an-safe-area` dejaba de apartar del arco y de crecer, y la pantalla
 * se quedaba negra.
 */
function defsFrom(def: Def, kind: 'directive' | 'pipe'): (() => Def[]) | null {
  if (def.dependencies == null) {
    return null
  }
  return () =>
    dependenciesOf(def)
      .map((dep) => (kind === 'pipe' ? dep.ɵpipe : (dep.ɵcmp ?? dep.ɵdir)))
      .filter((entry): entry is Def => entry != null)
}

export function hotRefresh(newRoot: Type<unknown>): boolean {
  if (!running) {
    return false
  }
  const previous = collect(running.root)
  const next = collect(newRoot as Declaration)
  if (previous.size !== next.size) {
    return false
  }
  for (const key of previous.keys()) {
    if (!next.has(key)) {
      return false
    }
  }

  for (const type of next.values()) {
    rewire(type, previous)
  }

  for (const [key, before] of previous) {
    const after = next.get(key)!
    // La misma clase, no una nueva que se le parece: viene de la mitad de
    // arriba del bundle, que no se vuelve a evaluar. Su definición no ha
    // cambiado, así que no hay nada que sustituir, y sustituirla igualmente no
    // sale gratis: `replaceMetadata` tira y rehace las vistas de esa clase, y
    // un componente con `<ng-content />` pierde por el camino lo que le
    // proyectaron. `an-safe-area` se quedaba vacío y con él la pantalla
    // entera.
    if (before === after) {
      continue
    }
    if (before.ɵcmp && after.ɵcmp) {
      // Angular mezcla la definición nueva sobre la vieja pero se queda con
      // las listas de directivas y pipes que hubiera en esta, así que es aquí
      // —y no en la nueva— donde hay que dejarlas rehechas.
      before.ɵcmp.directiveDefs = defsFrom(after.ɵcmp, 'directive')
      before.ɵcmp.pipeDefs = defsFrom(after.ɵcmp, 'pipe')
      const newDef = after.ɵcmp
      const newFactory = after.ɵfac
      replaceMetadata(
        before,
        (type: Declaration) => {
          type.ɵcmp = newDef
          type.ɵfac = newFactory
        },
        [],
        []
      )
    } else if (before.ɵdir && after.ɵdir) {
      // Una directiva no tiene vistas propias que rehacer: basta con dejarle
      // la definición nueva. Los componentes que la usan se rehacen en este
      // mismo barrido y la resuelven otra vez.
      before.ɵdir = after.ɵdir
      before.ɵfac = after.ɵfac
    } else if (before.ɵpipe && after.ɵpipe) {
      before.ɵpipe = after.ɵpipe
      before.ɵfac = after.ɵfac
    } else {
      // Lo que era componente ahora es directiva, o al revés. Cambiar eso en
      // caliente no tiene sentido.
      return false
    }
  }

  running = { root: running.root, app: running.app }
  return true
}
