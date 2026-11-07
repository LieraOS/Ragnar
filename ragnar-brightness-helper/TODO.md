# TODO — ragnar-brightness-helper

## Deutsch

- [ ] ragnar-core: legacy `ragnar-brightness-helper` + meson-Target
      beim nächsten Core-Rebuild entfernen (`Replaces` bis dahin nötig)
- [ ] Fallback für Geräte ohne logind-Session (direkter sysfs-Write,
      wenn Gruppenrechte es erlauben)
- [ ] Mehrere Backlight-Devices: Auswahl-Option (`--device`)

## English

- [ ] ragnar-core: drop the legacy `ragnar-brightness-helper` and its
      meson target on the next core rebuild (`Replaces` needed until
      then)
- [ ] Fallback for machines without a logind session (direct sysfs
      write when group permissions allow)
- [ ] Multiple backlight devices: `--device` selection option
