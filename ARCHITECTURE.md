# AORUS 16X (2024) - Reverse Engineering & Architecture Deep Dive

Bu belge, Gigabyte AORUS 16X (2024 - 9KG/9SG/AKG Serisi) laptop için **Gigabyte Control Center (GCC)** ve **RGB Fusion** yazılımlarını tamamen devreden çıkarıp, donanımı bağımsız, ultra hafif ve saf yerel (native) kodla kontrol etmek amacıyla yapılan tüm tersine mühendislik çalışmalarını, keşfedilen donanım protokollerini, yaşanan tuzakları ve çözüm mimarisini kayıt altına alır.

---

## 1. Amaç ve Kapsam

- **Problem:** Gigabyte Control Center (GCC), arka planda çalışan onlarca telemetri ve arka plan servisi (`GprocSrvc`, `GiMATE`, WMI döngüleri) nedeniyle 500+ MB RAM tüketmekte, CPU döngülerini işgal etmekte ve özellikle oyunlarda **micro-stutter** (%1 low FPS çöküşü) yaşatmaktadır. RGB Fusion modülü ise kararsız, ağır ve donanımı gereksiz tarama döngülerine sokan bir yapıya sahiptir.
- **Hedef:** 
  1. Klavyenin ve arkadaki 3 parçalı ışık barının (Aorus Beacon) doğrudan Windows HID API üzerinden sıfır arka plan yüküyle kontrol edilmesi.
  2. Fan modları (Quiet, Normal, Gaming, Turbo), manuel fan hızı ve %80 pil koruma sınırının doğrudan BIOS ACPI-WMI arabirimi üzerinden yönetilmesi.
  3. Tüm bu motorun **Rust** ve **Slint UI** ile tek bir taşınabilir `.exe` (15 MB RAM, %0.0 CPU) altında birleştirilmesi.
  4. Gelecekte Linux çekirdeğine (`ec_sys` ve `hidraw`) kolayca taşınabilir bir mimari kurulması.

---

## 2. Donanım Keşifleri & Tersine Mühendislik Çıktıları

### A. USB HID Aydınlatma Dünyası (Klavye & Arka Bar)

Laptop anakartındaki ITE/G-Style mikrodenetleyicisi dahili bir USB 2.0 köprüsü ile sisteme bağlıdır.

1. **Donanım Kimlikleri (VID & PID):**
   - **`VID_0414 & PID_8005` (Klavye Kontrolcüsü):** 3 bölgeli RGB klavye.
   - **`VID_0414 & PID_8012` (Arka Işık Barı / Aorus Beacon):** Arka hava çıkışındaki ana ışık barı, difüzör LED şeridi ve masaya yansıyan Aorus projeksiyon logosu.
   - *Kritik Tuzak:* İlk denemelerde bu iki PID ters eşlendiği için arka ışık butonuna basıldığında klavye renk değiştirmişti. `PID_8005` kesin olarak Klavye, `PID_8012` kesin olarak Arka Bardır.

2. **Paket Yapısı (70 Baytlık Feature Report):**
   Windows `hid.dll` üzerinden `HidD_SetFeature` çağrısıyla doğrudan gönderilir:
   ```text
   Byte 0:  0x00       (Report ID)
   Byte 1:  0x08       (Temel Işık Komutu) veya 0x0B (Strip/Logo Komutu)
   Byte 2:  0x00       (veya Zone belirteci)
   Byte 3:  0x01       (Mod: 1 = Saf Statik / Solid Color)
   Byte 4:  0x01       (Hız: 1)
   Byte 5:  Parlaklık  (0 - 100)
   Byte 6:  Renk Kodu  (1: Kırmızı, 2: Yeşil, 3: Sarı, 4: Mavi, 5: Turuncu, 6: Mor, 7: Beyaz, 0: Kapalı)
   Byte 7:  0x01       (Yön)
   Byte 8:  Sağlama    (Checksum = 0xFF - (Sum(Byte 1..7) & 0xFF))
   Byte 9..69: 0x00    (Sıfır dolgusu)
   ```

3. **Arka Barın 3 Bölgesi:**
   Arka ışık barı 3 bağımsız kısımdan oluşur:
   - Ana Işık Barı (`cmd = 0x08`)
   - LED Şeridi (`cmd = 0x0B`)
   - Projeksiyon Logosu (`cmd = 0x0B`)
   Tek bir komut gönderildiğinde diğer parçalar eski renginde kalır veya nefes alma (breathing) moduna geçer. Arka barın senkron ve tek tonda, pürüzsüz sabit kalması için hem `0x08` hem `0x0B` komutları `Type = 1 (Statik)` olarak eşzamanlı gönderilmelidir.

---

### B. Fan & Güç Dünyası (WMI ACPI ve EC Haritası)

Gigabyte mühendisleri, Windows uygulamalarının fan ve güç yönetimi yapabilmesi için anakart BIOS'una standart ACPI-WMI metotları gömmüştür.

1. **WMI İsim Uzayı & Sınıfları:**
   - **Adres:** `root\WMI`
   - **`GB_WMIACPI_Set` (Yazma Sınıfı):** Donanım ayarlarını değiştiren tüm metotlar bu sınıftadır.
   - **`GB_WMIACPI_Get` (Okuma Sınıfı):** Donanım sensör verilerini okuyan metotlar bu sınıftadır.

2. **Kritik Metot Parametre Kuralı:**
   `GB_WMIACPI_Set` içindeki metotlar parametresiz çağrılamaz. Her bir metot **`@{ Data = [byte]... }`** argüman sözlüğü bekler. Parametre verilmediğinde metot sessizce başarısız olur.

3. **Resmi Gigabyte Fan Modu Geçiş Zinciri (GCC Kaynak Kodundan Doğrulandı):**
   - **Quiet (Eco) Modu:**
     `SetFanModeNotify(@{ Data = [byte]0 })`
     `SetQuietMode(@{ Data = [byte]1 })`
     `SetFixedFanStatus(@{ Data = [byte]0 })`
     `SetAutoFanStatus(@{ Data = [byte]0 })`
     `SetThermalTppStatus(@{ Data = [byte]0 })`
   - **Normal (Dengeli) Modu:**
     `SetFanModeNotify(@{ Data = [byte]1 })`
     `SetQuietMode(@{ Data = [byte]0 })`
     `SetFixedFanStatus(@{ Data = [byte]0 })`
     `SetAutoFanStatus(@{ Data = [byte]0 })`
     `SetThermalTppStatus(@{ Data = [byte]0 })`
   - **Gaming (Oyun / Power) Modu:**
     `SetFanModeNotify(@{ Data = [byte]2 })`
     `SetQuietMode(@{ Data = [byte]0 })`
     `SetFixedFanStatus(@{ Data = [byte]0 })`
     `SetAutoFanStatus(@{ Data = [byte]1 })`
     `SetThermalTppStatus(@{ Data = [byte]1 })`
   - **Turbo (Maksimum Devir) Modu:**
     `SetFanModeNotify(@{ Data = [byte]3 })`
     `SetQuietMode(@{ Data = [byte]0 })`
     `SetAutoFanStatus(@{ Data = [byte]0 })`
     `SetFixedFanStatus(@{ Data = [byte]1 })`
     `SetStepFanStatus(@{ Data = [byte]1 })`
     `SetFixedFanSpeed(@{ Data = [byte]229 })`
     `SetGpuFanDuty(@{ Data = [byte]229 })`
   - **Manuel Sabit Fan Hızı:**
     `SetAutoFanStatus(@{ Data = [byte]0 })`  *(Otomatik BIOS eğrisini kesmek şarttır)*
     `SetFixedFanStatus(@{ Data = [byte]1 })`
     `SetStepFanStatus(@{ Data = [byte]1 })`
     `SetFixedFanSpeed(@{ Data = [byte]pwm })`
     `SetGpuFanDuty(@{ Data = [byte]pwm })`

4. **PWM Basamak Tablosu (16 Discrete Step):**
   Gigabyte'ın `ComData.dll` kütüphanesinden çıkarılan donanım hız tablosu:
   | Yüzde (%) | 25% | 30% | 35% | 40% | 45% | 50% | 55% | 60% | 65% | 70% | 75% | 80% | 85% | 90% | 95% | 100% |
   |:---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
   | **PWM (Byte)** | 57 | 68 | 80 | 91 | 103 | 114 | 125 | 137 | 148 | 160 | 171 | 183 | 194 | 206 | 217 | 229 |

5. **RPM Ters Bayt (Big Endian) Çözümü:**
   `GB_WMIACPI_Get.getRpm1` çıktısı olan `21002` değeri, Big Endian formatındaki `0x520A` sayısıdır.
   Düzeltme formülü: `((raw & 0xFF) << 8) | ((raw >> 8) & 0xFF)` -> `0x0A52` = **2642 RPM**.

---

## 3. Fabrika Fan Eğrileri (Sıcaklık vs Devir Verileri)

GCC'nin `ucNotebook.dll` içerisindeki `FanControlNb` ve `initQuietModel_vH` bayt dizilerinden çıkarılan fabrika eğrileri:

### A. Quiet (Sessiz / Eco) Modu Eğrisi
- **< 56°C:** %0 (0 RPM - Fanlar tamamen durur, 0 dB sessizlik)
- **59°C:** %22 (~1600 RPM)
- **62°C:** %25 (~1900 RPM - PWM 57)
- **65°C:** %29 (~2100 RPM)
- **68°C:** %35 (~2400 RPM - PWM 80)
- **71°C:** %38 (~2600 RPM)
- **74°C:** %38 (~2600 RPM)
- **82°C:** %44 (~3000 RPM - PWM 103)
- **90°C:** %54 (~3600 RPM - PWM 125)
- **92°C+:** %65 (~4200 RPM - PWM 148)

### B. Normal (Dengeli) Mod Eğrisi
- **< 45°C:** %25 (~1800 - 2000 RPM)
- **50°C:** %30 (~2200 RPM)
- **60°C:** %40 (~2700 RPM)
- **70°C:** %50 (~3300 RPM)
- **80°C:** %65 (~4200 RPM)
- **85°C+:** %80 - %100 (~4800 - 5400 RPM)

### C. Gaming (Power) Modu Eğrisi
- **Her zaman devrede:** Soğukken bile minimum %35 (~2400 RPM)
- **55°C:** %45 (~3000 RPM)
- **65°C:** %60 (~3900 RPM)
- **75°C:** %80 (~4800 RPM)
- **85°C+:** %100 (~5400 RPM)

### D. Turbo Modu
- Sıcaklıktan bağımsız olarak sabit **%100 (PWM 229, ~5400 RPM)**.

---

## 4. Uygulama Mimarisi & Dosya Haritası

```
C:\Users\sef\Desktop\Repolar\aorus-control/
├── Cargo.toml          # Rust paket tanımı (slint, windows-sys)
├── build.rs            # Slint UI arayüz derleyici
├── ui/
│   └── appwindow.slint # GPU hızlandırmalı modern karanlık tema arayüz
├── src/
│   ├── main.rs         # Arayüz döngüsü, olay bağlayıcıları, sensör zamanlayıcısı
│   ├── rgb.rs          # VID 0414 PID 8005/8012 saf USB HID paket sürücüsü
│   └── hardware.rs     # GB_WMIACPI_Set / Get WMI donanım motoru
├── README.md           # Genel tanıtım ve derleme rehberi
└── ARCHITECTURE.md     # Bu teknik referans belgesi
```

---

## 5. Sıradaki Geliştirmeler & Yol Haritası

1. **Ayar Kalıcılığı (Persistence):**
   - Ayarların `config.json` dosyasına yazılması.
   - Uygulama açılışında arka bar durumu, seçili renkler ve fan modunun bu dosyadan yüklenmesi.
2. **Slider Davranışı:**
   - 0..100 kaydırıcısının Slint tarafında tam çözünürlüklü ve tek kademeli çalışmasının sağlanması.
3. **Etkileşimli Fan Eğrisi Görselleştiricisi:**
   - 16 noktalı fabrika eğrilerinin arayüzde çizgi grafik veya adım tablosu olarak sunulması.
   - Özel fan eğrisi düzenleme kabiliyeti.
