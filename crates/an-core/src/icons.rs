//! Nombres comunes de icono, traducidos al SF Symbol que les toca.
//!
//! Vive en el núcleo por lo mismo que la tabla de colores: con más de un host
//! Apple —iOS, tvOS, visionOS y el reloj— dos copias serían dos sitios donde
//! `back` podría dejar de ser `chevron.left`, y un icono que cambia de dibujo
//! según la pantalla es un fallo que nadie ve hasta que lo ve un usuario.
//!
//! La lista es corta a propósito: cubre lo que lleva casi cualquier app —una
//! barra de pestañas, una cabecera— y para lo demás se escribe el nombre del
//! símbolo directamente, que son más de cinco mil y no tiene sentido
//! duplicarlos aquí.

/// El SF Symbol que corresponde a un nombre común. Lo que no está en la tabla
/// sale tal cual: es el nombre nativo del símbolo, que la plantilla puede
/// escribir directamente.
pub fn translate(name: &str) -> &str {
    match name {
        "home" => "house.fill",
        "search" => "magnifyingglass",
        "settings" => "gearshape.fill",
        "profile" | "account" => "person.crop.circle.fill",
        "back" => "chevron.left",
        "forward" => "chevron.right",
        "close" => "xmark",
        "add" => "plus",
        "remove" => "minus",
        "delete" => "trash",
        "edit" => "pencil",
        "share" => "square.and.arrow.up",
        "favorite" => "heart.fill",
        "star" => "star.fill",
        "menu" => "line.3.horizontal",
        "more" => "ellipsis",
        "check" => "checkmark",
        "info" => "info.circle",
        "warning" => "exclamationmark.triangle.fill",
        "refresh" => "arrow.clockwise",
        "calendar" => "calendar",
        "camera" => "camera.fill",
        "bell" => "bell.fill",
        "chat" => "bubble.left.fill",
        "mail" => "envelope.fill",
        "list" => "list.bullet",
        "play" => "play.fill",
        "pause" => "pause.fill",
        "download" => "arrow.down.circle",
        "upload" => "arrow.up.circle",
        "location" => "location.fill",
        "lock" => "lock.fill",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn los_nombres_comunes_se_traducen_y_los_nativos_pasan() {
        assert_eq!(super::translate("back"), "chevron.left");
        assert_eq!(super::translate("account"), "person.crop.circle.fill");
        // Un SF Symbol escrito a pelo no se toca: la tabla no es una lista
        // blanca, es un atajo.
        assert_eq!(super::translate("square.and.arrow.up"), "square.and.arrow.up");
        assert_eq!(super::translate("figure.run"), "figure.run");
    }
}
