package dev.angularnative;

import android.app.Activity;

import org.json.JSONObject;

/**
 * Lo que implementa un plugin de Android.
 *
 * <p>No declara su nombre: el nombre con el que JS lo invoca está en el {@code
 * angularNative.module} de su {@code package.json} y de ahí lo saca {@code an} al generar el
 * registro. Un solo sitio donde escribirlo es un sitio menos donde puedan dejar de coincidir.
 */
public interface AnPlugin {

    /**
     * La pantalla de la app, antes de la primera llamada.
     *
     * <p>Casi todo lo de Android pide un contexto, y un plugin no tiene de dónde sacarlo. Se le da
     * la Activity: de ahí salen tanto el contexto como el sitio donde presentar algo. Quien no la
     * necesite no implementa nada.
     */
    default void attach(Activity host) {}

    /**
     * Atiende una llamada.
     *
     * <p>{@code args} es lo que mandó JS, ya decodificado; si no mandó un objeto llega vacío en vez
     * de nulo. Se puede contestar en el acto o guardar el {@link AnPluginCall} y contestar más
     * tarde — es lo que separa leer el portapapeles de sacar una foto—, pero hay que contestar
     * siempre: un método que no existe se rechaza, no se ignora.
     */
    void call(String method, JSONObject args, AnPluginCall respond);
}
