## Which file do I need?

| Your computer | Download |
| --- | --- |
| **Windows**: most PCs (Intel or AMD processor) | [Tern_{v}_x64-setup.exe](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_x64-setup.exe) |
| **Windows on ARM**: Snapdragon PCs, for example Surface Pro X and Copilot+ PCs | [Tern_{v}_arm64-setup.exe](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_arm64-setup.exe) |
| **Mac with Apple silicon**: M1, M2, M3, M4 (most Macs from 2021 on) | [Tern_{v}_aarch64.dmg](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_aarch64.dmg) |
| **Mac with an Intel processor**: most Macs from 2020 and earlier | [Tern_{v}_x64.dmg](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_x64.dmg) |
| **Linux** (Intel or AMD) | [AppImage](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_amd64.AppImage) (any Linux) · [.deb](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_amd64.deb) (Ubuntu, Debian) · [.rpm](https://github.com/Car-Role/tern/releases/download/v{v}/Tern-{v}-1.x86_64.rpm) (Fedora, RHEL) |
| **Linux on ARM** | [AppImage](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_aarch64.AppImage) · [.deb](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_arm64.deb) · [.rpm](https://github.com/Car-Role/tern/releases/download/v{v}/Tern-{v}-1.aarch64.rpm) |

**Not sure which one?**
- Windows: open **Settings → System → About** and look at **System type**. "x64-based processor" means Windows; "ARM-based processor" means Windows on ARM.
- Mac: open the **Apple menu → About This Mac**. "Chip: Apple M…" means Apple silicon; "Processor: Intel" means Intel.

**For IT teams:** [Tern_{v}_x64_en-US.msi](https://github.com/Car-Role/tern/releases/download/v{v}/Tern_{v}_x64_en-US.msi) installs the Windows version with Group Policy or Intune.

**You can ignore** `latest.json` and the `.app.tar.gz` files in the list below. Installed copies of Tern use them to update themselves.

## Install notes

- **Mac:** Tern is not notarized yet. Open it once, then go to **System Settings → Privacy & Security** and click **Open Anyway**. If macOS still says it is damaged, run `xattr -dr com.apple.quarantine /Applications/Tern.app` in Terminal.
- **Windows:** if SmartScreen appears, click **More info**, then **Run anyway**.
- **Updates:** from 0.3.1 on, Tern updates itself. From 0.1 or 0.2, install the new version by hand once.
