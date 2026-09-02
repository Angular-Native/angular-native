import { ɵɵreplaceMetadata as replaceMetadata, type ApplicationRef, type Type } from '@angular/core'

/**
 * Hot refresh: changing the components' code without throwing the app away.
 *
 * When a file is saved, the new bundle is evaluated on top of the one already
 * running. Its top half —Angular, the framework— is not evaluated again, so
 * there is still only one copy of everything; the bottom half is, and it brings
 * new classes for the same components.
 *
 * Here the old ones are paired up with the new ones and Angular is handed the
 * new definition on top of the old class. Angular rebuilds the views but keeps
 * the component's instance, so the signals and the fields stay where they were:
 * you see the change without losing which screen you were on or what you had
 * typed.
 *
 * What it does not cover: changes in the top half —the framework, or a
 * dependency— and adding or removing components. In those cases it returns
 * `false` and the caller reloads the lot, which is the only honest thing to do:
 * any patch-up would leave the screen showing old code without saying so.
 */

/** What Angular hangs off the class. Not public API, but stable. */
interface Def {
  debugInfo?: { className: string; filePath: string } | null
  dependencies?: unknown
  directiveDefs?: unknown
  pipeDefs?: unknown
}

/** A compiled component, directive or pipe. */
interface Declaration extends Type<unknown> {
  ɵcmp?: Def
  ɵdir?: Def
  ɵpipe?: Def
  ɵfac?: unknown
}

/** The app that is running, so it can be compared with the one arriving. */
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
 * Where the class came from. The compiler emits `ɵsetClassDebugInfo` in
 * development builds, and it is the only thing that survives a recompile: the
 * class's name alone is not enough —two files can give theirs the same name— and
 * the object's identity is precisely what has just changed.
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
  // A dependency can arrive as a class or wrapped in an object, which is how
  // host directives are declared.
  return raw
    .map((entry) => (typeof entry === 'object' && entry !== null ? Reflect.get(entry, 'directive') : entry))
    .filter((entry): entry is Declaration => typeof entry === 'function')
}

/** Everything hanging off the root component, indexed by file and class. */
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
 * Points the new definition at the old classes.
 *
 * This is the step that preserves state. The instances that are mounted belong
 * to the old classes; if a parent's new template ordered the new classes of its
 * children to be built, every child would be born again and lose what was its
 * own.
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
 * The list of directives —or of pipes— a template is allowed to use.
 *
 * Angular resolves it exactly once, when the component is defined, and the
 * function it leaves behind closes over the dependencies of that moment:
 * changing `def.dependencies` afterwards does not touch it. And on a hot reload,
 * `mergeWithExistingDefinition` deliberately keeps whatever list the old
 * definition had, so the new bundle's does not get through either.
 *
 * With both doors shut, the only way for the new template to find its directives
 * is to rebuild the list here, out of the classes already re-pointed. A function
 * is returned rather than an array because those classes' definitions are being
 * changed in this very sweep: reading them now would give the old ones.
 *
 * Leaving it `null` —which is what used to happen— does not make it recompute:
 * it leaves it empty for ever. And it is barely noticeable: with no directives,
 * `[backgroundColor]` stops being `an-view`'s input and goes through
 * `Renderer2.setProperty`, which here ends up at the same `setProp`, so the
 * screen goes on painting the same. What does not survive is what a directive
 * does and a prop does not —hooking up a listener, writing styles from the
 * host— and that was first seen on the watch: `an-safe-area` stopped keeping
 * clear of the bezel and stopped growing, and the screen went black.
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
    // The very same class, not a new one that looks like it: it comes from the
    // bundle's top half, which is not evaluated again. Its definition has not
    // changed, so there is nothing to replace, and replacing it anyway is not
    // free: `replaceMetadata` throws away and rebuilds that class's views, and a
    // component with an `<ng-content />` loses along the way whatever was
    // projected into it. `an-safe-area` was left empty and the whole screen with
    // it.
    if (before === after) {
      continue
    }
    if (before.ɵcmp && after.ɵcmp) {
      // Angular merges the new definition over the old one but keeps whatever
      // directive and pipe lists the latter had, so this —and not the new one—
      // is where they have to be left rebuilt.
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
      // A directive has no views of its own to rebuild: handing it the new
      // definition is enough. The components that use it are rebuilt in this
      // same sweep and resolve it again.
      before.ɵdir = after.ɵdir
      before.ɵfac = after.ɵfac
    } else if (before.ɵpipe && after.ɵpipe) {
      before.ɵpipe = after.ɵpipe
      before.ɵfac = after.ɵfac
    } else {
      // What was a component is now a directive, or the other way round.
      // Changing that hot makes no sense.
      return false
    }
  }

  running = { root: running.root, app: running.app }
  return true
}
