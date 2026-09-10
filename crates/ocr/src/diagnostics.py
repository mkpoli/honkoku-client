"""Run the installed package's diagnostics and repair verified model files."""
import sys
from honkoku_ocr import doctor, models

if sys.argv[1] == "doctor":
    print(doctor.render(doctor.report()))
elif sys.argv[1] == "verify":
    try:
        models.ensure(offline=True, quiet=True, digest=True)
    except Exception as error:
        print(str(error))
        sys.exit(1)
elif sys.argv[1] == "repair":
    directory = models.model_dir()
    for name in models.specification(models.DEFAULT_VERSION).files.values():
        path = directory / name
        if path.is_file():
            try:
                models.verify(path, name, digest=True)
            except (OSError, RuntimeError):
                path.unlink()
    models.ensure(quiet=True, digest=True)
