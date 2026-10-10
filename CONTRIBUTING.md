# دليل المساهمة · Contributing

[العربية](#العربية) · [English](#english)

<div dir="rtl">

## العربية

شكرًا لاهتمامك بتطوير أوان. لاقتراح ميزة أو الإبلاغ عن مشكلة افتح [Issue](../../issues)، وللمساهمة بالكود أرسل Pull Request.

### المتطلبات

- [Node.js 22](https://nodejs.org)
- [Rust](https://rustup.rs) (الإصدار المستقر)
- [متطلبات Tauri لويندوز](https://tauri.app/start/prerequisites/): أدوات Microsoft C++ Build Tools وWebView2
- Python 3 مع مكتبة numpy، لإعادة توليد الأصوات فقط

### البناء والتشغيل

```bash
npm ci                       # تثبيت أدوات البناء
npm run dev                  # تشغيل التطبيق أثناء التطوير
npm test                     # اختبارات الواجهة
cd src-tauri && cargo test   # اختبارات المنطق
npm run build                # بناء المثبّت في src-tauri/target/release/bundle/nsis
```

### بنية المشروع

```
src/                 الواجهة: HTML وCSS وJavaScript دون إطار عمل أو أداة تجميع
  styles/themes.css  جميع السمات
  js/pages/          صفحات المؤقت والسجل والأهداف والإعدادات
  locales/           النصوص العربية والإنجليزية
src-tauri/           الجانب الأصلي بلغة Rust: محرك المؤقت، والتخزين في SQLite،
                     وعلبة النظام، والوضع المصغّر، والاختصارات، والإشعارات
scripts/             مصادر الشعار، ومولّدات الأيقونات والأصوات، وتحديث رقم الإصدار،
                     واختبارات ويندوز
tests/               اختبارات الواجهة
docs/                لقطات الشاشة والشعار
```

### قواعد عامة

- الكود والتعليقات وأسماء الملفات ورسائل الـ commit بالإنجليزية.
- كل نص يظهر في الواجهة يُضاف إلى `src/locales/ar.js` و`src/locales/en.js` معًا، وتتحقق الاختبارات من ذلك.
- يشغّل GitHub Actions على ويندوز مع كل رفع: الاختبارات، وبناء المثبّت، واختبار التثبيت وإلغائه، واختبار تشغيل التطبيق.

### إضافة سمة جديدة

جميع السمات في ملف واحد: `src/styles/themes.css`. انسخ إحدى الكتل، وغيّر معرّفها (`data-theme`) واسمها (`--theme-name-ar` و`--theme-name-en`) وألوانها، فتظهر في الإعدادات تلقائيًا.

### الشعار والأيقونات

مصدر الشعار `scripts/icon.svg`، ومصدر أيقونة علبة النظام `scripts/tray-icon.svg`. بعد تعديل أيٍّ منهما شغّل `npm run icons`. وتفاصيل صور المثبّت ونسخة الشعار البديلة في [docs/logo](docs/logo/README.md).

### الأصوات

تُولَّد أصوات التنبيه برمجيًا بالأمر `python3 scripts/gen_sounds.py`، وكلها أصوات طبيعية بلا آلات موسيقية ولا ألحان.

### نشر إصدار جديد (للمشرف على المشروع)

```bash
npm run version -- 1.1.0   # يحدّث رقم الإصدار في جميع ملفات المشروع
git commit -am "Release 1.1.0"
git tag v1.1.0
git push && git push origin v1.1.0
```

عند رفع الوسم يتحقق GitHub Actions من تطابقه مع رقم الإصدار، ويشغّل الاختبارات، ويبني المثبّت ويختبره، ثم ينشئ صفحة الإصدار ويرفق بها المثبّت مع ملاحظات التثبيت من `.github/release-notes.md`.

ويمكن النشر دون رفع وسم: بعد رفع التعديلات افتح تبويب **Actions** ← **Release** ← **Run workflow**، واكتب الوسم (مثل `v1.1.0`)، فيُنشأ الوسم والإصدار تلقائيًا.

</div>

---

## English

Thank you for your interest in Awan. To suggest a feature or report a problem, open an [issue](../../issues); to contribute code, send a pull request.

### Requirements

- [Node.js 22](https://nodejs.org)
- [Rust](https://rustup.rs) (stable)
- The [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/): Microsoft C++ Build Tools and WebView2
- Python 3 with numpy, only to regenerate the sounds

### Build and run

```bash
npm ci                       # install the build tools
npm run dev                  # run the app in development
npm test                     # UI tests
cd src-tauri && cargo test   # core logic tests
npm run build                # build the installer into src-tauri/target/release/bundle/nsis
```

### Project layout

```
src/                 UI: HTML, CSS and JavaScript (no framework, no bundler)
  styles/themes.css  all themes
  js/pages/          timer, history, goals, settings
  locales/           Arabic and English text
src-tauri/           native side (Rust): timer engine, SQLite storage,
                     system tray, mini window, shortcuts, notifications
scripts/             logo sources, icon and sound generators, version bump,
                     Windows tests
tests/               UI tests
docs/                screenshots and logo
```

### Guidelines

- Code, comments, file names and commit messages are in English.
- Every string shown in the UI goes into both `src/locales/ar.js` and `src/locales/en.js`; the tests check this.
- GitHub Actions runs on Windows for every push: the tests, the installer build, an install and uninstall test, and a test that launches the app.

### Add a theme

All themes live in one file, `src/styles/themes.css`. Copy a block and change its id (`data-theme`), its names (`--theme-name-ar` and `--theme-name-en`) and its colors; it appears in Settings automatically.

### Logo and icons

The logo comes from `scripts/icon.svg` and the system tray icon from `scripts/tray-icon.svg`. After editing either, run `npm run icons`. The installer images and the alternate logo are described in [docs/logo](docs/logo/README.md).

### Sounds

The alert sounds are generated with `python3 scripts/gen_sounds.py`. They are all natural sounds, with no musical instruments or melodies.

### Publish a release (maintainers)

```bash
npm run version -- 1.1.0   # update the version in every project file
git commit -am "Release 1.1.0"
git tag v1.1.0
git push && git push origin v1.1.0
```

Pushing the tag makes GitHub Actions check that it matches the app version, run the tests, build and test the installer, then create the release with the installer attached and the install notes from `.github/release-notes.md`.

You can also publish without pushing a tag: after pushing your commits, open **Actions** → **Release** → **Run workflow** and enter the tag (for example `v1.1.0`); the tag and the release are created for you.
