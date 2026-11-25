# ragnarok

Easter egg with real teeth: `ragnarok` shows every error the
system recorded — journald error-priority records merged with
the classic `/var/log` and per-user Xorg logs that journald
never sees — grouped by source and sorted by error count.

Also answers to `ragnarock` and `ragnarök` (symlinks).

```
ragnarok                      all errors, current boot
ragnarok lightdm              filtered by source name
ragnarok -b -2                boot two boots ago
ragnarok --all                all recorded boots
ragnarok lightdm -o out.txt   also write a plain-text report
```

Shipped inside `ragnar-core` — it exists only on a Ragnar
system, which is the point of the easter egg.

[Deutsche Version](README_de.md)
