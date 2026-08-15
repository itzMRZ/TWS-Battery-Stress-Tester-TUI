# Family protocols

How nearby open-source tools talk to devices, and which protocol facts (UUIDs, channels, frames) this project can use without copying GPL or AGPL code.

Adapters are Family probes, not per-model branches. Live tests may pin one MAC. Parsers must not.

## Status

| Family | Module | Transport | Status |
| --- | --- | --- | --- |
| Soundcore / Anker | `src/cells/soundcore.rs` | RFCOMM | Live soak: P30i |
| Samsung | `src/cells/samsung.rs` | SPP `FE`/`EE` + `FD`/`DD` | Live soak: Buds2 Pro |
| Apple | `src/cells/apple.rs` | AAP L2CAP `0x1001` | Live soak: AirPods Pro |
| Beats | same AAP module | AAP | Parser only; no live Beats soak |
| Sony | `src/cells/sony.rs` | MDR / Tandem RFCOMM | Fixtures only |
| Nothing / CMF | `src/cells/nothing.rs` | `0x55` RFCOMM | Fixtures only |
| Bose | `src/cells/bose.rs` | BMAP RFCOMM | Fixtures only |
| Oppo / OnePlus / Realme | `src/cells/opo.rs` | OPO / HeyMelody RFCOMM | Fixtures only |

A green fixture test is not a hardware confirmation. Live soaks in the table were run on Linux in this project.

## Do not copy

Useful to *read* for opcodes and captures. Their code is GPL or AGPL:

| Project | License | Family |
| --- | --- | --- |
| [OpenSCQ30](https://github.com/Oppzippy/OpenSCQ30) | GPL-3.0 | Soundcore |
| [GalaxyBudsClient](https://github.com/timschneeb/GalaxyBudsClient) | GPL-3.0 | Samsung |
| [LibrePods](https://github.com/kavishdevar/librepods) | GPL-3.0 | Apple/Beats |
| [OpenPods](https://github.com/adolfintel/OpenPods/) | GPL | Apple/Beats BLE ads |
| [Gadgetbridge](https://codeberg.org/Freeyourgadget/Gadgetbridge) | AGPL | Sony, Huawei, others |
| [OpenFreebuds](https://github.com/melianmiko/OpenFreebuds) | GPL | Huawei/Honor |

## MIT / Apache sources for wired adapters

### Sony (MDR, Tandem RFCOMM)

- [mdr-protocol](https://github.com/AndreasOlofsson/mdr-protocol): Tandem `0x3E`...`0x3C`, escape `0x3D` then `byte & 0xEF`, DataMdr `0x0C`, ACK `0x01`, battery types `0x00` pack / `0x02` L+R / `0x03` case. GET `0x10`, RET `0x11`.
- [Plutoberth/SonyHeadphonesClient](https://github.com/Plutoberth/SonyHeadphonesClient) (MIT, archived) and [mos9527/SonyHeadphonesClient](https://github.com/mos9527/SonyHeadphonesClient) (MIT).
- [AmitRajput-Dev/SonyBridge](https://github.com/AmitRajput-Dev/SonyBridge) (MIT): v1 UUID `96CC203E-...`, v2 `956C7B26-...`.
- [Leonard013/sony-ult-ctl](https://github.com/Leonard013/sony-ult-ctl) (MIT): channel 18; v2 hello `RET [0x01, 0x00, 0x03, ...]`; v2 battery `GET [0x22, 0x00]`. On v1, `0x22` is power-off. Send it only after a v2 hello.

### Nothing / CMF (`0x55` RFCOMM)

- [SoaOaoS/something-x](https://github.com/SoaOaoS/something-x) (MIT): GET battery `0xC007`, events `0xE001`, types 2/3/4 = L/R/case, `% = val & 0x7F`.
- [Dospacite/NothingLinux protocol.md](https://github.com/Dospacite/NothingLinux/blob/main/docs/protocol.md): control UUID `aeac4a03-dff5-498f-843a-34487cf133eb`, channel 15, same `0x55` envelope, CRC16-ARC.
- [bharadwaj Ear (2) write-up](https://bharadwajraju.com/posts/nothing-ear-2-on-linux/): published ANC frame used as the CRC fixture.

### Bose (BMAP RFCOMM)

- [aaronsb/bosectl](https://github.com/aaronsb/bosectl) (MIT): GET `[2, 2, 0x01, 0x00]`, `resp[4]` is percent. Channels 2 (QC Ultra 2) and 8 (QC35). Headphones emit **pack** only.

### Oppo / OnePlus / Realme (OPO RFCOMM)

One codec for the three Families (HeyMelody).

- [Swastik36/OPPO-Earbuds PROTOCOL.md](https://github.com/Swastik36/OPPO-Earbuds/blob/main/docs/PROTOCOL.md) (MIT): SOF `0xAA`, varint length, header `00 00`, command LE. SPP UUID `00001107-D102-11E1-9B23-00025B00A5A5`, channel 15. Battery GET `0x0105` / RET `0x8105` or `0x8106`. Packed `[id, val]` (1/2/3 = L/R/case) or ASCII CSV with charging offset +100.
- [Leaf-lsgtky/OppoPods](https://github.com/Leaf-lsgtky/OppoPods): GET `0x0106` / RET `0x8106`, same id/val encoding.
- [Cracking OPOv1](https://aasheesh.vercel.app/blog/oneplus-buds): BLE GATT `0000079A-D102-11E1-9B23-00025B00A5A5`; RFCOMM needs no Hello/Register (PROTOCOL.md). ANC capture `AA 0A 00 00 04 04 ...` is the framing fixture, not a cell.

## Families without an adapter yet

### Google Pixel Buds (Maestro)

- [qzed/pbpctrl](https://github.com/qzed/pbpctrl) (MIT/Apache): Maestro over RFCOMM, UUID `25e97ff7-24ce-4c4c-8951-f764a708f7b5`. Protobuf + HDLC, heavier than a byte parser. Fast Pair message stream (`df21fe2c-...`) is a lighter cross-OEM path. Do not map `FE2C` to `Family::Google`.

### Huawei / Honor (SPP `0x5A`)

- [gist](https://gist.github.com/melianmiko/02f7c6a550808e38d9b6760fb688e125): SOF `0x5A`, cmd `0108` battery, param 2 = L/R/case bytes, CRC16-XMODEM, often channel 16. Implement from those facts. Do not copy OpenFreebuds.

### Xiaomi / Redmi

- [CesurPolat/MiBudsClient](https://github.com/CesurPolat/MiBudsClient) is SKU-shaped. Community captures use `FE DC BA` on RFCOMM ~28. Need a probe folder before a Xiaomi adapter.

### Apple BLE ads (not a soak)

- [hudsonbrendon/apple-ble](https://github.com/hudsonbrendon/apple-ble) (MIT): Continuity ads, 10% nibbles. Useful when A2DP is down. Connected soaks already use AAP.

### Samsung extra

- [tommie/pygalaxybuds](https://github.com/tommie/pygalaxybuds) (MIT): SPP framing reference. This project already parses `FE`/`FD`.

## Adding the next Family

See [CONTRIBUTING.md](../CONTRIBUTING.md). Probe first. Missing cells stay unknown. Headphones get `pack`, not invented left/right.
