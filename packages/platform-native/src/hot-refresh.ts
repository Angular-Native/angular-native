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
    if (before.ɵcmp && after.ɵcmp) {
      // Los `defs` resueltos se guardan en caché la primera vez que se usan.
      // Angular conserva la caché vieja al mezclar, y entonces un componente
      // recién añadido a una plantilla no aparecería. Vaciarla antes la hace
      // recalcularse a partir de las dependencias que se acaban de reapuntar.
      before.ɵcmp.directiveDefs = null
      before.ɵcmp.pipeDefs = null
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
