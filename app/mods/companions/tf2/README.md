# Team Fortress 2 setup (no mod needed)

TF2 can write its console — including the kill feed — to a file. VibeLoop
tails it. Setup is one launch option:

1. Steam → right-click Team Fortress 2 → Properties → Launch Options
2. Add: `-condebug -conclearlog`
3. Open `team_fortress2.lua` in VibeLoop's mods folder and set `MY_NICK`
   to your exact in-game name (the kill feed is text — without your name
   the mod can't tell your kills from anyone else's).

That's it. `-condebug` writes `tf/console.log`; `-conclearlog` empties it
on every launch so it never grows forever. This is a plain engine feature —
no VAC concerns whatsoever.
