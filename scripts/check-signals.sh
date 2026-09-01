#!/usr/bin/env bash
# Nada de decoradores: entradas, salidas y consultas van con señales.
#
# No es cuestión de gusto. Un `@Input() set` corre en el momento exacto en que
# Angular escribe la entrada, así que el orden de las escrituras depende del
# orden de los bindings de la plantilla; una señal se lee cuando alguien la lee,
# y quien manda al núcleo es un `effect`. Mezclar las dos cosas en el mismo
# árbol es pedir que algo llegue en un orden distinto según quién lo escribió.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== señales"
fail=0
prohibido() {
  # Se busca la llamada, no la palabra: `@Input` aparece en los comentarios que
  # explican por qué ya no se usa.
  found="$(grep -rn --include='*.ts' -- "$1" packages examples || true)"
  if [ -n "$found" ]; then
    echo "  FALLO $2"
    echo "$found" | sed 's/^/         /'
    fail=1
  else
    echo "  ok   $2"
  fi
}

prohibido '@Input(' 'ninguna entrada con decorador; van con input()'
prohibido '@Output(' 'ninguna salida con decorador; van con output()'
prohibido 'new EventEmitter' 'ningún EventEmitter; las salidas son output()'
prohibido '@ViewChild\|@ViewChildren\|@ContentChild\|@ContentChildren' \
  'ninguna consulta con decorador; van con viewChild() y contentChild()'
prohibido '@HostBinding\|@HostListener' \
  'nada de HostBinding ni HostListener; van en el host del decorador'

exit "$fail"
