package dev.angularnative;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.LruCache;
import android.view.MotionEvent;
import android.view.View;

import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Map.
 *
 * Android ships none in the platform: Google's lives in Play Services, which is
 * a dependency with an API key and this build does not use Gradle. So here
 * OpenStreetMap tiles are drawn onto a `Canvas`.
 *
 * It is a genuine native view —not a browser in disguise— but it is not the
 * system map either, and that is worth saying: it brings no directions, no
 * search, and no blue dot for where you are. It draws the world and lets you
 * drag it.
 */
public final class AnMapView extends View {

    private static final int TILE = 256;
    /** The OSM tiles ask you to identify yourself; without this they return 403. */
    private static final String AGENT = "angular-native/0.1 (example)";

    private final ExecutorService fetcher = Executors.newFixedThreadPool(4);
    /** A quarter of the app's memory, which is what Android recommends. */
    private final LruCache<String, Bitmap> cache =
            new LruCache<String, Bitmap>((int) (Runtime.getRuntime().maxMemory() / 4096)) {
                @Override
                protected int sizeOf(String key, Bitmap value) {
                    return value.getByteCount() / 1024;
                }
            };

    private final Paint paint = new Paint(Paint.FILTER_BITMAP_FLAG);
    private double latitude;
    private double longitude;
    private double zoom = 12;
    private float lastX;
    private float lastY;

    public AnMapView(Context context) {
        super(context);
        setBackgroundColor(Color.rgb(28, 34, 48));
    }

    public void setCenter(double latitude, double longitude) {
        this.latitude = latitude;
        this.longitude = longitude;
        invalidate();
    }

    public void setZoom(double zoom) {
        this.zoom = Math.max(1, Math.min(19, zoom));
        invalidate();
    }

    // --- the Mercator projection, which is the one the tiles use

    private static double lonToX(double lon, int z) {
        return (lon + 180.0) / 360.0 * (1 << z);
    }

    private static double latToY(double lat, int z) {
        double rad = Math.toRadians(lat);
        return (1 - Math.log(Math.tan(rad) + 1 / Math.cos(rad)) / Math.PI) / 2 * (1 << z);
    }

    private static double xToLon(double x, int z) {
        return x / (1 << z) * 360.0 - 180.0;
    }

    private static double yToLat(double y, int z) {
        double n = Math.PI - 2 * Math.PI * y / (1 << z);
        return Math.toDegrees(Math.atan(Math.sinh(n)));
    }

    @Override
    protected void onDraw(Canvas canvas) {
        int z = (int) Math.round(zoom);
        double centerX = lonToX(longitude, z);
        double centerY = latToY(latitude, z);
        // Top left corner, in pixels of the whole world.
        double originX = centerX * TILE - getWidth() / 2.0;
        double originY = centerY * TILE - getHeight() / 2.0;

        int firstCol = (int) Math.floor(originX / TILE);
        int firstRow = (int) Math.floor(originY / TILE);
        int cols = (int) Math.ceil((double) getWidth() / TILE) + 1;
        int rows = (int) Math.ceil((double) getHeight() / TILE) + 1;
        int limit = 1 << z;

        for (int row = firstRow; row < firstRow + rows; row++) {
            for (int col = firstCol; col < firstCol + cols; col++) {
                if (row < 0 || row >= limit) {
                    continue;
                }
                // Across, the world wraps around; vertically it does not.
                int wrapped = ((col % limit) + limit) % limit;
                Bitmap tile = tile(z, wrapped, row);
                if (tile == null) {
                    continue;
                }
                float left = (float) (col * TILE - originX);
                float top = (float) (row * TILE - originY);
                canvas.drawBitmap(tile, left, top, paint);
            }
        }
    }

    /** The tile if it is already there; if not, it is requested and redrawn on arrival. */
    private Bitmap tile(int z, int x, int y) {
        final String key = z + "/" + x + "/" + y;
        Bitmap hit = cache.get(key);
        if (hit != null) {
            return hit;
        }
        cache.put(key, PENDING);
        fetcher.execute(
                () -> {
                    Bitmap bitmap = download(z, x, y);
                    if (bitmap != null) {
                        cache.put(key, bitmap);
                        postInvalidate();
                    }
                });
        return null;
    }

    /**
     * A 1x1 bitmap that marks "already requested".
     *
     * Without this, every redraw would request the tiles that are on their way
     * again, and dragging the map would fire off hundreds of identical
     * downloads.
     */
    private static final Bitmap PENDING =
            Bitmap.createBitmap(1, 1, Bitmap.Config.ALPHA_8);

    private Bitmap download(int z, int x, int y) {
        HttpURLConnection connection = null;
        try {
            URL url = new URL("https://tile.openstreetmap.org/" + z + "/" + x + "/" + y + ".png");
            connection = (HttpURLConnection) url.openConnection();
            connection.setRequestProperty("User-Agent", AGENT);
            connection.setConnectTimeout(8000);
            connection.setReadTimeout(8000);
            try (InputStream stream = connection.getInputStream()) {
                return BitmapFactory.decodeStream(stream);
            }
        } catch (Exception error) {
            return null;
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                lastX = event.getX();
                lastY = event.getY();
                return true;
            case MotionEvent.ACTION_MOVE: {
                int z = (int) Math.round(zoom);
                // How far the finger has moved, in pixels, turned into degrees.
                double x = lonToX(longitude, z) * TILE - (event.getX() - lastX);
                double y = latToY(latitude, z) * TILE - (event.getY() - lastY);
                longitude = xToLon(x / TILE, z);
                latitude = yToLat(y / TILE, z);
                lastX = event.getX();
                lastY = event.getY();
                invalidate();
                return true;
            }
            default:
                return super.onTouchEvent(event);
        }
    }
}
