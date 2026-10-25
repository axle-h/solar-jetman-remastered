package com.ax_h.solarjetman;

import org.libsdl.app.SDLActivity;

// SDL loads these libraries in order and calls SDL_main in the last, src/app/android.rs.
public class MainActivity extends SDLActivity {
    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "solar_jetman_android" };
    }
}
