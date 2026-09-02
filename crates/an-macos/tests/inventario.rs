//! El inventario de `support.rs`, por dentro.
//!
//! Corre en cualquier máquina y sin abrir ninguna ventana: `support` está fuera
//! de `cfg(target_os = "macos")` justo para esto.
//!
//! Aquí no se repite la lista de primitivas. Que la tabla cubra el enum entero
//! lo comprueba `scripts/check-macos.py`, que lee los dos ficheros: copiar aquí
//! los veinticinco nombres sería añadir la cuarta lista que hay que mantener a
//! mano, que es lo que `check-kinds.sh` y `check-styles.sh` existen para evitar.
//! Lo que se mira desde Rust es lo que un script no puede ver sin volver a
//! escribir el compilador: que cada entrada dice algo, que ninguna se repite y
//! que lo que no es una vista no está.

use an_core::NodeKind;
use an_macos::support::{support, Support, KNOWN_EVENTS, SUPPORT};

/// Un `Missing` o un `Assembled` sin motivo es lo mismo que no decir nada: el
/// informe sale con un hueco y quien lo lee se queda igual.
#[test]
fn lo_que_no_es_nativo_viene_con_su_motivo() {
    for (kind, sop) in SUPPORT {
        let texto = match sop {
            Support::Native(nombre) => nombre,
            Support::Assembled(motivo)
            | Support::Missing(motivo)
            | Support::Elsewhere(motivo) => motivo,
        };
        assert!(!texto.trim().is_empty(), "{kind:?} no dice qué hay detrás");
    }
}

/// `support()` devuelve la primera coincidencia, así que una entrada repetida
/// deja una segunda que nadie lee nunca: cambiarla no tendría ningún efecto y
/// tampoco daría ningún error.
#[test]
fn ninguna_primitiva_aparece_dos_veces() {
    for (kind, _) in SUPPORT {
        assert_eq!(
            SUPPORT.iter().filter(|(k, _)| k == kind).count(),
            1,
            "{kind:?} aparece dos veces en la tabla"
        );
    }
}

/// `RawText` nunca es una vista. Si algún día lo fuera, esta prueba avisa antes
/// de que el host lo monte como una caja vacía.
#[test]
fn el_texto_crudo_no_es_una_vista() {
    assert!(support(NodeKind::RawText).is_none());
}

/// El host consulta `is_known_event` con una búsqueda lineal; un nombre
/// repetido no rompe nada, pero delata que la lista se editó a ciegas y la
/// siguiente edición puede quitar solo una de las dos copias.
#[test]
fn los_nombres_de_evento_no_se_repiten() {
    for evento in KNOWN_EVENTS {
        assert_eq!(
            KNOWN_EVENTS.iter().filter(|e| *e == evento).count(),
            1,
            "«{evento}» aparece dos veces en KNOWN_EVENTS"
        );
    }
}

/// Hacia dónde va cada signo en un deslizamiento de AppKit.
///
/// Es lo único de este host que no se puede comprobar corriéndolo: hace falta
/// un trackpad y una mano encima. Lo que sí se puede es no equivocarse al
/// copiar lo que dice Apple, que es lo que esta prueba fija. De `NSEvent.h`:
/// «A non-0 deltaX will represent a horizontal swipe, -1 for swipe right and 1
/// for swipe left. A non-0 deltaY will represent a vertical swipe, -1 for
/// swipe down and 1 for swipe up.»
#[test]
fn el_signo_del_deslizamiento_es_el_que_dice_appkit() {
    use an_macos::support::{swipe_bit, swipe_direction};

    assert_eq!(swipe_direction(-1.0, 0.0).map(|(_, n)| n), Some("swipeRight"));
    assert_eq!(swipe_direction(1.0, 0.0).map(|(_, n)| n), Some("swipeLeft"));
    assert_eq!(swipe_direction(0.0, -1.0).map(|(_, n)| n), Some("swipeDown"));
    assert_eq!(swipe_direction(0.0, 1.0).map(|(_, n)| n), Some("swipeUp"));
    // Sin dirección no hay deslizamiento: el evento se pasa a la cadena.
    assert!(swipe_direction(0.0, 0.0).is_none());

    // Y el bit que sale de la dirección es el que se suscribe con ese nombre,
    // o una vista escucharía una dirección y recibiría otra.
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        let (bit, nombre) = swipe_direction(dx, dy).expect("hay dirección");
        assert_eq!(swipe_bit(nombre), Some(bit), "«{nombre}» se suscribe con otro bit");
    }
}
