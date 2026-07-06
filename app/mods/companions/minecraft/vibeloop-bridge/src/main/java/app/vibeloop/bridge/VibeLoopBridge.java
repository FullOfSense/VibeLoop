package app.vibeloop.bridge;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Locale;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.network.ClientPlayerEntity;

/**
 * VibeLoop bridge: watches the local player each client tick and appends one
 * JSON line per event to ~/.vibeloop/minecraft.jsonl — a launcher-independent
 * location, so it works identically for the vanilla launcher, Prism,
 * CurseForge instances, etc. VibeLoop tails that file.
 *
 * Client-side only: works on any server, changes no gameplay.
 */
public class VibeLoopBridge implements ClientModInitializer {
    private static final Path OUT =
            Path.of(System.getProperty("user.home"), ".vibeloop", "minecraft.jsonl");

    private float lastHealth = -1;
    private int lastLevel = -1;
    private boolean wasDead = false;

    @Override
    public void onInitializeClient() {
        try {
            Files.createDirectories(OUT.getParent());
            Files.writeString(OUT, ""); // fresh file per game launch
        } catch (IOException ignored) {
        }

        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            ClientPlayerEntity player = client.player;
            if (player == null) {
                // Left the world: forget baselines so rejoining is silent.
                lastHealth = -1;
                lastLevel = -1;
                wasDead = false;
                return;
            }

            float health = player.getHealth();
            float max = Math.max(player.getMaxHealth(), 1.0f);
            boolean dead = player.isDead();

            if (dead && !wasDead) {
                emit("{\"e\":\"died\"}");
            } else if (!dead && wasDead) {
                emit("{\"e\":\"respawn\"}");
                lastHealth = -1; // health snaps back to full; don't call it a heal
            } else if (lastHealth >= 0 && health < lastHealth - 0.01f) {
                float frac = (lastHealth - health) / max;
                emit(String.format(Locale.ROOT, "{\"e\":\"dmg\",\"f\":%.4f}", frac));
            }
            wasDead = dead;
            lastHealth = health;

            int level = player.experienceLevel;
            if (lastLevel >= 0 && level > lastLevel) {
                emit("{\"e\":\"levelup\",\"n\":" + level + "}");
            }
            lastLevel = level;
        });
    }

    private void emit(String jsonLine) {
        try {
            Files.write(
                    OUT,
                    (jsonLine + "\n").getBytes(StandardCharsets.UTF_8),
                    StandardOpenOption.CREATE,
                    StandardOpenOption.APPEND);
        } catch (IOException ignored) {
            // Never let haptics bookkeeping break the game.
        }
    }
}
