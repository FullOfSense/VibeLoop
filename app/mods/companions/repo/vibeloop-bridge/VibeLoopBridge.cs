using System;
using System.Globalization;
using System.IO;
using BepInEx;
using HarmonyLib;

namespace VibeLoop.Repo
{
    /// <summary>
    /// VibeLoop bridge for R.E.P.O.: Harmony postfixes on the local player's
    /// health events, appended as JSON lines to ~/.vibeloop/repo.jsonl.
    /// Local-player only (PlayerAvatar.isLocal) — teammates' pain is theirs.
    /// </summary>
    [BepInPlugin("app.vibeloop.repo", "VibeLoop Bridge", "1.0.0")]
    public class VibeLoopBridge : BaseUnityPlugin
    {
        internal static readonly string OutPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            ".vibeloop", "repo.jsonl");

        private void Awake()
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(OutPath));
                File.WriteAllText(OutPath, ""); // fresh file per game launch
            }
            catch
            {
                // Haptics must never break the game.
            }
            Harmony.CreateAndPatchAll(typeof(Patches));
            Logger.LogInfo("VibeLoop bridge writing to " + OutPath);
        }

        internal static void Emit(string jsonLine)
        {
            try
            {
                File.AppendAllText(OutPath, jsonLine + "\n");
            }
            catch
            {
            }
        }
    }

    internal static class Patches
    {
        private static bool IsLocal(PlayerHealth health)
        {
            var avatar = Traverse.Create(health).Field("playerAvatar").GetValue<PlayerAvatar>();
            return avatar != null && Traverse.Create(avatar).Field("isLocal").GetValue<bool>();
        }

        [HarmonyPostfix, HarmonyPatch(typeof(PlayerHealth), "Hurt")]
        private static void Hurt(PlayerHealth __instance, int damage)
        {
            if (damage <= 0 || !IsLocal(__instance))
            {
                return;
            }
            int max = Math.Max(Traverse.Create(__instance).Field("maxHealth").GetValue<int>(), 1);
            string frac = ((double)damage / max).ToString("0.####", CultureInfo.InvariantCulture);
            VibeLoopBridge.Emit("{\"e\":\"dmg\",\"f\":" + frac + "}");
        }

        [HarmonyPostfix, HarmonyPatch(typeof(PlayerHealth), "Death")]
        private static void Death(PlayerHealth __instance)
        {
            if (IsLocal(__instance))
            {
                VibeLoopBridge.Emit("{\"e\":\"died\"}");
            }
        }

        [HarmonyPostfix, HarmonyPatch(typeof(PlayerHealth), "Heal")]
        private static void Heal(PlayerHealth __instance, int healAmount)
        {
            if (healAmount > 0 && IsLocal(__instance))
            {
                VibeLoopBridge.Emit("{\"e\":\"heal\"}");
            }
        }

        // Fires when an extraction point completes — the shared "we did it".
        [HarmonyPostfix, HarmonyPatch(typeof(ExtractionPoint), "StateComplete")]
        private static void Extracted()
        {
            VibeLoopBridge.Emit("{\"e\":\"extracted\"}");
        }
    }
}
