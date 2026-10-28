# FEDEK Ders Değerlendirme

Ege Üniversitesi Arkeoloji Bölümü'nün FEDEK akreditasyonu için kullandığı
**ÖÇ-PÇ (öğrenme çıktısı – program çıktısı) Excel şablonunun** masaüstü karşılığı.
Hesaplar şablonla birebir aynıdır; uygulama aynı düzende `.xlsx` dosyası da üretir.

## Ne yapar

1. **Ders bilgileri** – kod/ad, sorumlu, dönem, tür, program (PÇ listesi), eşik yüzdesi.
2. **Öğrenme çıktıları** – ÖÇ metinleri, her ÖÇ için tek PÇ ve 1-5 ilişki düzeyi.
3. **Sınavlar ve puanlar** – istenildiği kadar değerlendirme (ara sınav, final, ödev…);
   soru başına ÖÇ etiketi, puan (100 / soru sayısı) ve eşik puanı; öğrenci listesi
   (yapıştırılabilir), F2/S- işaretleri, puan girişi.
4. **Sonuçlar** – ÖÇ ve PÇ yüzdeleri, veri kontrolleri (ölçülmeyen ÖÇ, boş eşik, tutarsız
   örneklem…), değerlendirme metinleri, **Rapor (PDF)** ve **Excel'e aktar**.

**Yazdırılabilir rapor**: "Rapor (PDF)" düğmesi okunaklı bir A4 rapor üretir ve açar
(ders bilgileri, PÇ sağlanma tablosu, ÖÇ tablosu, her sınav için soru tablosu,
değerlendirme metinleri, imza satırı; istenirse ek olarak tüm öğrenci puanları,
çok soruluysa yatay sayfada). Rapor uygulamanın içinde Typst dizgi motoruyla
üretilir; Word, Excel, yazıcı sürücüsü dışında hiçbir şey gerekmez.

**Bologna bilgi paketi bağlantısı** yeter: dersin ebp.ege.edu.tr sayfasının bağlantısı
yapıştırıldığında ders kodu/adı, sorumlusu, ÖÇ listesi, PÇ listesi ve ÖÇ-PÇ ilişki
matrisi indirilir; her ÖÇ için matristeki en yüksek düzeyli PÇ önerilir (eşitlikler
işaretlenir) ve uygulanmadan önce matris üzerinde düzeltilebilir. Program sayfasının
bağlantısı ise PÇ listesini doldurur. İndirme arka planda yapılır; internet yoksa
uygulama elle girişle aynen çalışır.

Eski Excel dosyaları **içe aktarılır**; veriler kullanıcının uygulama klasöründe JSON olarak
kendiliğinden kaydedilir (Linux: `~/.local/share/fedekdegerlendirme`, Windows:
`%APPDATA%\Arkeoloji\FEDEK Degerlendirme\data`, Mac:
`~/Library/Application Support/tr.Arkeoloji.FEDEK-Degerlendirme`).

## Yapı

```
crates/fedek-core   hesap motoru, Excel içe/dışa aktarma, Bologna sayfası ayrıştırma, kontroller
crates/fedek-report Typst ile A4 PDF rapor
crates/fedek-app    egui/eframe masaüstü arayüzü
fixtures/           anonimleştirilmiş gerçek şablon; "altın" testlerin kaynağı
```

## Windows'ta kurulum (öğretim elemanları için)

1. GitHub'daki **Releases** sayfasından `fedek-app.exe` dosyasını indirin.
2. Dosyayı istediğiniz klasöre (ör. Masaüstü) koyup çift tıklayın. Kurulum yoktur.
3. İlk açılışta Windows SmartScreen "tanınmayan yayıncı" uyarısı verirse
   **Daha fazla bilgi → Yine de çalıştır** deyin (dosya imzalı değildir).
4. Verileriniz `%APPDATA%\Arkeoloji\FEDEK Degerlendirme\data` klasöründe saklanır.

## Mac'te kurulum

1. **Releases** sayfasından `fedek-app-macos.zip` dosyasını indirip açın; içinden
   **FEDEK Degerlendirme.app** çıkar. Uygulamalar klasörüne ya da Masaüstüne taşıyın.
   Tek dosya hem Apple Silicon (M1/M2/M3…) hem Intel Mac'lerde çalışır (macOS 11+).
2. Uygulama Apple tarafından imzalanmadığı için ilk açılışta çift tıklama engellenir.
   Uygulamaya **sağ tıklayıp Aç** deyin ve çıkan pencerede yine **Aç**'ı seçin; bu bir
   kez yeter. Hâlâ açılmıyorsa Sistem Ayarları → Gizlilik ve Güvenlik altındaki
   **Yine de Aç** düğmesini kullanın.
3. Verileriniz `~/Library/Application Support/tr.Arkeoloji.FEDEK-Degerlendirme`
   klasöründe saklanır.

Her `v*` etiketi GitHub Actions'ta Windows, Linux ve macOS sürümlerini derleyip
Releases'a ekler.

## Derleme

Rust (stable) gerekir: <https://rustup.rs>

```sh
cargo test                       # motor, içe/dışa aktarma ve yuvarlak-tur testleri
cargo run -p fedek-app           # uygulamayı çalıştır
cargo build --release -p fedek-app   # tek dosya: target/release/fedek-app(.exe)
```

Geliştirici anahtarları: `--open-last`, `--step info|outcomes|assessments|results`, `--programs`,
`--bologna <url>` (verilen Bologna ders sayfasından yeni ders açar).

Windows, Linux ve macOS derlemeleri `.github/workflows/build.yml` ile GitHub Actions
üzerinde yapılır (her sürüm için `fedek-app.exe`, `fedek-app` ve `fedek-app-macos.zip`). Windows'ta WebView, Java vb.
hiçbir ek bileşen gerekmez; `.exe` tek başına çalışır.

## Doğrulama

* `tests/golden.rs`: gerçek şablonun Excel'in kendi hesapladığı tüm hücre değerleri
  motorla karşılaştırılır (PÇ yüzdeleri, ÖÇ ortalamaları, soru başına oranlar).
* `tests/roundtrip.rs`: dışa aktar → içe aktar kayıpsızdır; yazılan önbellek değerleri motorla aynıdır.
* `bologna.rs` birim testleri: `fixtures/bologna_*.html` (gerçek ebp.ege.edu.tr sayfaları)
  üzerinden ÖÇ/PÇ listeleri, matris değerleri, boş hücreler ve eşitlik önerileri;
  `examples/bologna_fetch.rs` canlı sayfayı indirip ayrıştırır.
* `fedek-report` testleri: altın şablon çok sayfalı PDF'e dizilir; Typst için özel
  karakterler içeren metinler (`"`, `\`, `#`, `[`, `$`, `//`) sorunsuz kaçışlanır; boş ders
  bile tek sayfa rapor verir. `examples/report_fixture.rs` bir .xlsx dosyasını PDF yapar.
* `examples/compare_cached.rs`: herhangi bir (ör. Excel/LibreOffice'te yeniden hesaplanmış)
  şablon dosyasını motorla karşılaştırır. Dışa aktarılan dosya LibreOffice ile yeniden
  hesaplatıldığında 68/68 hücre eşleşmiştir.

## Bilinçli olarak korunan şablon kuralları

* Hiçbir soruyla ölçülmeyen ÖÇ, bağlı olduğu PÇ'nin paydasına girer, payına 0 katar.
* F2 (devamsız) ve S- (örneklem dışı) öğrenciler paydadan düşülür.
* Soru oranları değerlendirme içinde ve değerlendirmeler arasında ağırlıksız ortalanır.
* Boş ilişki düzeyi 0 sayılır; eşik boşsa hiçbir öğrenci sağlamış sayılmaz.
