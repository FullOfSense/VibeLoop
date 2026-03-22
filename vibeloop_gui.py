#!/usr/bin/env python3
"""
VibeLoop GUI
============
Launcher for VibeLoop integrations with room management.

Requirements:
    pip install customtkinter buttplug websockets
"""

import asyncio
import json
import subprocess
import sys
import threading
import time
import tkinter as tk
from tkinter import messagebox
import customtkinter as ctk

# ─── Theme ───────────────────────────────────────────────────────────────────

ctk.set_appearance_mode("dark")
ctk.set_default_color_theme("blue")

BG          = "#0f0f0f"
BG_CARD     = "#181818"
BG_INPUT    = "#222222"
ACCENT      = "#e8552a"          # warm orange-red — distinct, not purple
ACCENT_DIM  = "#7a2e16"
TEXT        = "#f0ede8"
TEXT_DIM    = "#7a7570"
TEXT_HINT   = "#3d3a37"
BORDER      = "#2a2a2a"
SUCCESS     = "#4caf82"
WARNING     = "#e8a02a"

FONT_TITLE  = ("Georgia", 28, "bold")
FONT_SUB    = ("Georgia", 12, "italic")
FONT_LABEL  = ("Courier New", 11)
FONT_SMALL  = ("Courier New", 10)
FONT_INPUT  = ("Courier New", 13)
FONT_MONO   = ("Courier New", 10)

GAMES = [
    {
        "id":     "osu",
        "name":   "osu!",
        "desc":   "Hit judgements, misses, fail & pass",
        "status": "available",
        "folder": "Osu",
        "script": "vibeloop_osu_rewarding.py",   # default
        "variants": [
            {"label": "Rewarding", "script": "vibeloop_osu_rewarding.py"},
            {"label": "Punishing", "script": "vibeloop_osu_punishing.py"},
        ],
    },
    {
        "id":     "lol",
        "name":   "League of Legends",
        "desc":   "Damage, kills, abilities",
        "status": "soon",
        "folder": None,
        "script": None,
        "variants": [],
    },
    {
        "id":     "minecraft",
        "name":   "Minecraft",
        "desc":   "Environment, damage, events",
        "status": "soon",
        "folder": None,
        "script": None,
        "variants": [],
    },
]


# ─── App ─────────────────────────────────────────────────────────────────────

class VibeLoopApp(ctk.CTk):
    def __init__(self):
        super().__init__()

        self.title("VibeLoop")
        self.geometry("560x700")
        self.resizable(False, False)
        self.configure(fg_color=BG)

        self.selected_game = None
        self.mode          = tk.StringVar(value="local")   # local / host / client
        self.process       = None   # running subprocess
        self._client_poll  = None

        self._build_ui()

    # ── UI Construction ───────────────────────────────────────────────────────

    def _build_ui(self):
        # ── Header ──
        header = ctk.CTkFrame(self, fg_color=BG_CARD, corner_radius=0, height=90)
        header.pack(fill="x")
        header.pack_propagate(False)

        ctk.CTkLabel(
            header, text="VibeLoop",
            font=FONT_TITLE, text_color=TEXT,
        ).place(x=28, y=18)
        ctk.CTkLabel(
            header, text="real-time haptic feedback for games",
            font=FONT_SUB, text_color=TEXT_DIM,
        ).place(x=30, y=58)

        # Accent bar under header
        ctk.CTkFrame(self, fg_color=ACCENT, height=2, corner_radius=0).pack(fill="x")

        # ── Scrollable content ──
        self.scroll = ctk.CTkScrollableFrame(self, fg_color=BG, corner_radius=0)
        self.scroll.pack(fill="both", expand=True, padx=0, pady=0)

        self._section("SELECT GAME")
        self._build_game_selector()

        self._section("MODE")
        self._build_mode_selector()

        self._build_relay_section()

        self._section("LAUNCH")
        self._build_launch_section()

        self._build_status_bar()

    def _section(self, label: str):
        f = ctk.CTkFrame(self.scroll, fg_color=BG, corner_radius=0)
        f.pack(fill="x", padx=24, pady=(18, 4))
        ctk.CTkLabel(f, text=label, font=FONT_SMALL,
                     text_color=TEXT_DIM).pack(anchor="w")
        ctk.CTkFrame(f, fg_color=BORDER, height=1,
                     corner_radius=0).pack(fill="x", pady=(4, 0))

    def _build_game_selector(self):
        self.game_buttons  = {}
        self.game_variants = {}   # game_id → CTkOptionMenu

        for game in GAMES:
            card = ctk.CTkFrame(
                self.scroll, fg_color=BG_CARD,
                corner_radius=6, border_width=1, border_color=BORDER,
            )
            card.pack(fill="x", padx=24, pady=4)

            left = ctk.CTkFrame(card, fg_color="transparent")
            left.pack(side="left", fill="both", expand=True, padx=14, pady=12)

            name_color = TEXT if game["status"] == "available" else TEXT_DIM
            ctk.CTkLabel(left, text=game["name"],
                         font=("Courier New", 13, "bold"),
                         text_color=name_color).pack(anchor="w")
            ctk.CTkLabel(left, text=game["desc"],
                         font=FONT_SMALL, text_color=TEXT_DIM).pack(anchor="w")

            # Variant dropdown (only for games with variants)
            if game.get("variants") and game["status"] == "available":
                variant_var = tk.StringVar(value=game["variants"][0]["label"])
                self.game_variants[game["id"]] = {
                    "var":      variant_var,
                    "variants": game["variants"],
                }
                ctk.CTkOptionMenu(
                    left,
                    values=[v["label"] for v in game["variants"]],
                    variable=variant_var,
                    font=FONT_SMALL,
                    fg_color=BG_INPUT,
                    button_color=ACCENT,
                    button_hover_color=ACCENT_DIM,
                    dropdown_fg_color=BG_CARD,
                    text_color=TEXT,
                    width=160,
                    height=26,
                    command=lambda val, g=game: self._on_variant_change(g, val),
                ).pack(anchor="w", pady=(6, 0))

            right = ctk.CTkFrame(card, fg_color="transparent")
            right.pack(side="right", padx=14, pady=12)

            if game["status"] == "available":
                btn = ctk.CTkButton(
                    right, text="Select", width=72, height=30,
                    font=FONT_SMALL, corner_radius=4,
                    fg_color=BG_INPUT, hover_color=ACCENT,
                    border_width=1, border_color=BORDER,
                    text_color=TEXT,
                    command=lambda g=game: self._select_game(g),
                )
                btn.pack()
                self.game_buttons[game["id"]] = btn
            else:
                ctk.CTkLabel(right, text="soon",
                             font=FONT_SMALL, text_color=TEXT_HINT).pack()

    def _build_mode_selector(self):
        frame = ctk.CTkFrame(self.scroll, fg_color=BG_CARD,
                             corner_radius=6, border_width=1, border_color=BORDER)
        frame.pack(fill="x", padx=24, pady=4)

        modes = [
            ("local",  "Local only",   "No relay. Just your machine."),
            ("host",   "Host a room",  "Broadcast to remote viewers."),
            ("client", "Join a room",  "Receive from a remote host."),
        ]

        for value, label, hint in modes:
            row = ctk.CTkFrame(frame, fg_color="transparent")
            row.pack(fill="x", padx=14, pady=6)

            ctk.CTkRadioButton(
                row, text=label, variable=self.mode, value=value,
                font=("Courier New", 12, "bold"), text_color=TEXT,
                fg_color=ACCENT, hover_color=ACCENT_DIM,
                command=self._on_mode_change,
            ).pack(side="left")
            ctk.CTkLabel(row, text=hint, font=FONT_SMALL,
                         text_color=TEXT_DIM).pack(side="left", padx=(10, 0))

    def _build_relay_section(self):
        self.relay_frame = ctk.CTkFrame(
            self.scroll, fg_color=BG_CARD,
            corner_radius=6, border_width=1, border_color=BORDER,
        )
        # Shown/hidden by mode selection
        self.relay_frame.pack(fill="x", padx=24, pady=4)

        # ── Server URL ──
        row = ctk.CTkFrame(self.relay_frame, fg_color="transparent")
        row.pack(fill="x", padx=14, pady=(14, 6))
        ctk.CTkLabel(row, text="SERVER URL", font=FONT_SMALL,
                     text_color=TEXT_DIM).pack(anchor="w")
        self.entry_server = ctk.CTkEntry(
            row, placeholder_text="ws://yourserver:8765",
            font=FONT_INPUT, height=36, corner_radius=4,
            fg_color=BG_INPUT, border_color=BORDER, text_color=TEXT,
        )
        self.entry_server.pack(fill="x", pady=(4, 0))

        # ── Room code ──
        row2 = ctk.CTkFrame(self.relay_frame, fg_color="transparent")
        row2.pack(fill="x", padx=14, pady=6)
        ctk.CTkLabel(row2, text="ROOM CODE", font=FONT_SMALL,
                     text_color=TEXT_DIM).pack(anchor="w")
        self.entry_room = ctk.CTkEntry(
            row2, placeholder_text="e.g. HYPE",
            font=FONT_INPUT, height=36, corner_radius=4,
            fg_color=BG_INPUT, border_color=BORDER, text_color=TEXT,
        )
        self.entry_room.pack(fill="x", pady=(4, 0))

        # ── Password ──
        row3 = ctk.CTkFrame(self.relay_frame, fg_color="transparent")
        row3.pack(fill="x", padx=14, pady=(6, 14))
        ctk.CTkLabel(row3, text="PASSWORD  (optional)",
                     font=FONT_SMALL, text_color=TEXT_DIM).pack(anchor="w")
        self.entry_password = ctk.CTkEntry(
            row3, placeholder_text="leave blank for no password",
            font=FONT_INPUT, height=36, corner_radius=4,
            fg_color=BG_INPUT, border_color=BORDER, text_color=TEXT,
            show="•",
        )
        self.entry_password.pack(fill="x", pady=(4, 0))

        # ── Client count (host only) ──
        self.client_count_frame = ctk.CTkFrame(
            self.relay_frame, fg_color="transparent"
        )
        self.client_count_label = ctk.CTkLabel(
            self.client_count_frame,
            text="● 0 viewers connected",
            font=FONT_SMALL, text_color=TEXT_DIM,
        )
        self.client_count_label.pack(anchor="w", padx=14, pady=(0, 12))

        self._on_mode_change()

    def _build_launch_section(self):
        frame = ctk.CTkFrame(self.scroll, fg_color="transparent")
        frame.pack(fill="x", padx=24, pady=(4, 8))

        self.btn_launch = ctk.CTkButton(
            frame, text="LAUNCH", height=46,
            font=("Courier New", 14, "bold"),
            fg_color=ACCENT, hover_color=ACCENT_DIM,
            corner_radius=4, text_color=TEXT,
            command=self._launch,
        )
        self.btn_launch.pack(fill="x")

        self.btn_stop = ctk.CTkButton(
            frame, text="STOP", height=36,
            font=FONT_SMALL,
            fg_color=BG_INPUT, hover_color="#3a1a1a",
            corner_radius=4, text_color=TEXT_DIM,
            border_width=1, border_color=BORDER,
            command=self._stop,
            state="disabled",
        )
        self.btn_stop.pack(fill="x", pady=(6, 0))

        # Log output
        self._section("LOG")
        self.log_box = ctk.CTkTextbox(
            self.scroll, height=160, font=FONT_MONO,
            fg_color=BG_CARD, text_color=TEXT_DIM,
            border_color=BORDER, border_width=1,
            corner_radius=4, wrap="word",
        )
        self.log_box.pack(fill="x", padx=24, pady=(4, 24))
        self.log_box.configure(state="disabled")

    def _build_status_bar(self):
        bar = ctk.CTkFrame(self, fg_color=BG_CARD, height=28, corner_radius=0)
        bar.pack(fill="x", side="bottom")
        bar.pack_propagate(False)
        self.status_label = ctk.CTkLabel(
            bar, text="Ready.", font=FONT_MONO, text_color=TEXT_DIM,
        )
        self.status_label.pack(side="left", padx=12)

    # ── Interactions ──────────────────────────────────────────────────────────

    def _on_variant_change(self, game: dict, label: str):
        # Update the game's active script to match chosen variant
        for v in game.get("variants", []):
            if v["label"] == label:
                game["script"] = v["script"]
                break
        # If this game is already selected, refresh selected_game too
        if self.selected_game and self.selected_game["id"] == game["id"]:
            self.selected_game = game

    def _select_game(self, game: dict):
        self.selected_game = game
        for gid, btn in self.game_buttons.items():
            if gid == game["id"]:
                btn.configure(fg_color=ACCENT, text_color=TEXT,
                              border_color=ACCENT, text="✓ Selected")
            else:
                btn.configure(fg_color=BG_INPUT, text_color=TEXT,
                              border_color=BORDER, text="Select")
        self._set_status(f"Game selected: {game['name']}")

    def _on_mode_change(self):
        mode = self.mode.get()
        if mode == "local":
            self.relay_frame.pack_forget()
        else:
            self.relay_frame.pack(fill="x", padx=24, pady=4,
                                  after=self.scroll.winfo_children()[4])
            if mode == "host":
                self.client_count_frame.pack(fill="x")
            else:
                self.client_count_frame.pack_forget()

    def _launch(self):
        mode = self.mode.get()

        # Validation
        if mode in ("host", "client"):
            server = self.entry_server.get().strip()
            room   = self.entry_room.get().strip()
            if not server or not room:
                messagebox.showwarning("Missing fields",
                    "Please enter a server URL and room code.")
                return

        if mode in ("local", "host"):
            if not self.selected_game:
                messagebox.showwarning("No game selected",
                    "Please select a game first.")
                return
            if not self.selected_game["script"]:
                messagebox.showinfo("Not available",
                    f"{self.selected_game['name']} is not yet available.")
                return

        # Resolve script path relative to gui location
        import os
        gui_dir    = os.path.dirname(os.path.abspath(__file__))
        game_dir   = os.path.join(gui_dir, self.selected_game["folder"]) \
                     if self.selected_game["folder"] else gui_dir
        script     = os.path.join(game_dir, self.selected_game["script"])

        if not os.path.exists(script):
            messagebox.showerror("Script not found",
                f"Could not find:\n{script}\n\nMake sure the file is in the {self.selected_game['folder']} folder.")
            return

        # Build command
        if mode == "local":
            cmd = [sys.executable, script]

        elif mode == "host":
            server   = self.entry_server.get().strip()
            room     = self.entry_room.get().strip().upper()
            password = self.entry_password.get().strip()
            cmd = [
                sys.executable, script,
                "--relay", server,
                "--room",  room,
            ]
            if password:
                cmd += ["--password", password]

        elif mode == "client":
            server   = self.entry_server.get().strip()
            room     = self.entry_room.get().strip().upper()
            password = self.entry_password.get().strip()
            cmd = [
                sys.executable, os.path.join(gui_dir, "vibeloop_client.py"),
                "--server", server,
                "--room",   room,
            ]
            if password:
                cmd += ["--password", password]

        # Launch subprocess from the script's own directory
        try:
            self.process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
                cwd=game_dir,
            )
        except FileNotFoundError as e:
            messagebox.showerror("Launch failed", str(e))
            return

        self.btn_launch.configure(state="disabled", text="RUNNING...")
        self.btn_stop.configure(state="normal")
        self._clear_log()
        self._set_status("Running...")

        # Stream logs from subprocess
        threading.Thread(target=self._stream_logs, daemon=True).start()

        # Poll client count if hosting
        if mode == "host":
            self._start_client_count_poll(
                self.entry_server.get().strip(),
                self.entry_room.get().strip().upper(),
                self.entry_password.get().strip() or None,
            )

    def _stop(self):
        if self.process:
            self.process.terminate()
            self.process = None
        if self._client_poll:
            self._client_poll = None
        self.btn_launch.configure(state="normal", text="LAUNCH")
        self.btn_stop.configure(state="disabled")
        self._set_status("Stopped.")
        self._log("— stopped —")

    # ── Log streaming ─────────────────────────────────────────────────────────

    def _stream_logs(self):
        if not self.process:
            return
        for line in self.process.stdout:
            self._log(line.rstrip())
        # Process ended
        self.after(0, self._on_process_ended)

    def _on_process_ended(self):
        self.btn_launch.configure(state="normal", text="LAUNCH")
        self.btn_stop.configure(state="disabled")
        self._set_status("Stopped.")

    def _log(self, text: str):
        def _write():
            self.log_box.configure(state="normal")
            self.log_box.insert("end", text + "\n")
            self.log_box.see("end")
            self.log_box.configure(state="disabled")
        self.after(0, _write)

    def _clear_log(self):
        self.log_box.configure(state="normal")
        self.log_box.delete("1.0", "end")
        self.log_box.configure(state="disabled")

    # ── Client count polling (host mode) ─────────────────────────────────────

    def _start_client_count_poll(self, server: str, room: str, password: str | None):
        self._client_poll = True
        threading.Thread(
            target=self._poll_client_count,
            args=(server, room, password),
            daemon=True,
        ).start()

    def _poll_client_count(self, server: str, room: str, password: str | None):
        """
        Connects as a secondary host connection just to receive client count pings.
        Uses a simple asyncio loop in a thread.
        """
        import websockets as ws_lib

        async def _run():
            try:
                async with ws_lib.connect(server) as ws:
                    hs = {"role": "host", "room": room + "_monitor"}
                    if password:
                        hs["password"] = password
                    # We can't actually join as host twice, so just ping the
                    # running process log for client count — read from log output
                    # This is a placeholder; count is shown in subprocess log
                    pass
            except Exception:
                pass

        # Actually, read client count from the log output instead
        # The server sends "clients=N" in pong responses — we parse the log
        while self._client_poll and self.process:
            time.sleep(2)
            # Count is visible in the log already; update label from log parsing
            # is complex — for now show a pulsing indicator
            self.after(0, self._update_client_label)

    def _update_client_label(self):
        # Parse latest count from log box content
        content = self.log_box.get("1.0", "end")
        count = 0
        for line in reversed(content.splitlines()):
            if "clients=" in line.lower() or "viewers" in line.lower():
                import re
                m = re.search(r'clients[=: ]+(\d+)', line, re.IGNORECASE)
                if m:
                    count = int(m.group(1))
                    break
        dot_color = SUCCESS if count > 0 else TEXT_DIM
        self.client_count_label.configure(
            text=f"● {count} viewer{'s' if count != 1 else ''} connected",
            text_color=dot_color,
        )

    # ── Status bar ────────────────────────────────────────────────────────────

    def _set_status(self, text: str):
        self.status_label.configure(text=text)


# ─── Entry point ─────────────────────────────────────────────────────────────

if __name__ == "__main__":
    app = VibeLoopApp()
    app.mainloop()
