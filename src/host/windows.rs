//! Windows host facts for a probe. Not a Host adapter; scan and playback still fail here.

use super::cmd_output;

pub fn probe_logs() -> Vec<(String, String)> {
    vec![
        (
            "windows-os".into(),
            cmd_output(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"
$ErrorActionPreference = 'Continue'
[Environment]::OSVersion | Format-List | Out-String
Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, OSArchitecture, BuildNumber | Format-List | Out-String
$PSVersionTable | Format-List | Out-String
"#,
                ],
            ),
        ),
        (
            "windows-pnp-bluetooth".into(),
            cmd_output(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"
$ErrorActionPreference = 'Continue'
if (Get-Command Get-PnpDevice -ErrorAction SilentlyContinue) {
  Get-PnpDevice -Class Bluetooth -ErrorAction SilentlyContinue |
    Format-Table Status, Class, FriendlyName, InstanceId -AutoSize | Out-String -Width 200
} else {
  'Get-PnpDevice missing'
}
"#,
                ],
            ),
        ),
        (
            "windows-pnp-audio".into(),
            cmd_output(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"
$ErrorActionPreference = 'Continue'
Get-CimInstance Win32_SoundDevice | Select-Object Name, Status, PNPDeviceID | Format-List | Out-String
if (Get-Command Get-PnpDevice -ErrorAction SilentlyContinue) {
  Get-PnpDevice -ErrorAction SilentlyContinue |
    Where-Object { $_.FriendlyName -match 'audio|headphone|headset|bud|speaker|bluetooth' } |
    Format-Table Status, Class, FriendlyName, InstanceId -AutoSize | Out-String -Width 200
}
"#,
                ],
            ),
        ),
        (
            "windows-bth-services".into(),
            cmd_output(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"
$ErrorActionPreference = 'Continue'
Get-Service | Where-Object { $_.Name -match 'bth|Bluetooth' } |
  Format-Table Status, Name, DisplayName -AutoSize | Out-String
Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services' -ErrorAction SilentlyContinue |
  Where-Object { $_.PSChildName -match 'bth|BTH|Bluetooth' } |
  Select-Object -ExpandProperty PSChildName | Out-String
"#,
                ],
            ),
        ),
    ]
}
