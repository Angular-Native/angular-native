package dev.angularnative;

import android.app.DatePickerDialog;
import android.app.TimePickerDialog;
import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.widget.TextView;

import java.util.Calendar;

/**
 * Selector de fecha y hora.
 *
 * En Android el selector no vive en la pantalla como en iOS: es un diálogo que
 * se abre al tocar. Lo que hay en la pantalla es la fecha escrita, formateada
 * con el idioma y la zona del dispositivo, y el diálogo lo pone el sistema.
 */
public final class AnDateField extends TextView {

    public interface OnChanged {
        void onChanged(double millis);
    }

    private OnChanged listener;
    private final Calendar calendar = Calendar.getInstance();
    private String mode = "date";

    public AnDateField(Context context) {
        super(context);
        setGravity(Gravity.CENTER);
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        setTextColor(Color.WHITE);
        GradientDrawable fondo = new GradientDrawable();
        fondo.setCornerRadius(1000f);
        fondo.setColor(Color.argb(28, 255, 255, 255));
        setBackground(fondo);
        setOnClickListener(v -> open());
        refresh();
    }

    public void setListener(OnChanged listener) {
        this.listener = listener;
    }

    public void setMode(String mode) {
        this.mode = mode;
        refresh();
    }

    public void setMillis(long millis) {
        calendar.setTimeInMillis(millis);
        refresh();
    }

    private void open() {
        if ("time".equals(mode)) {
            openTime();
            return;
        }
        new DatePickerDialog(
                        getContext(),
                        (picker, year, month, day) -> {
                            calendar.set(Calendar.YEAR, year);
                            calendar.set(Calendar.MONTH, month);
                            calendar.set(Calendar.DAY_OF_MONTH, day);
                            if ("dateAndTime".equals(mode)) {
                                // Con los dos, la hora se pregunta después: el
                                // sistema no trae un diálogo que haga las dos
                                // cosas a la vez.
                                openTime();
                            } else {
                                commit();
                            }
                        },
                        calendar.get(Calendar.YEAR),
                        calendar.get(Calendar.MONTH),
                        calendar.get(Calendar.DAY_OF_MONTH))
                .show();
    }

    private void openTime() {
        new TimePickerDialog(
                        getContext(),
                        (picker, hour, minute) -> {
                            calendar.set(Calendar.HOUR_OF_DAY, hour);
                            calendar.set(Calendar.MINUTE, minute);
                            commit();
                        },
                        calendar.get(Calendar.HOUR_OF_DAY),
                        calendar.get(Calendar.MINUTE),
                        android.text.format.DateFormat.is24HourFormat(getContext()))
                .show();
    }

    private void commit() {
        refresh();
        if (listener != null) {
            listener.onChanged(calendar.getTimeInMillis());
        }
    }

    private void refresh() {
        java.text.DateFormat format;
        if ("time".equals(mode)) {
            format = android.text.format.DateFormat.getTimeFormat(getContext());
        } else if ("dateAndTime".equals(mode)) {
            format = java.text.DateFormat.getDateTimeInstance(
                    java.text.DateFormat.MEDIUM, java.text.DateFormat.SHORT);
        } else {
            format = android.text.format.DateFormat.getDateFormat(getContext());
        }
        setText(format.format(calendar.getTime()));
    }
}
