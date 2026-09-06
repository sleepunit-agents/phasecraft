"""Real Tauri/WebKit smoke test. Run under xvfb-run on Linux, after building."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
from selenium import webdriver
from selenium.webdriver.common.options import ArgOptions
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait, Select

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="phasecraft-native-") as work:
    work = Path(work)
    project = work / "night-maps"
    shutil.copytree(root / "templates/project", project)
    composition = project / "compositions/techno.toml"
    composition.write_text(composition.read_text().replace("tempo = 132", "tempo = 400").replace("phrase_bars = 4", "phrase_bars = 1"))
    journey = project / "compositions/garage-journey.toml"
    journey.write_text(journey.read_text().replace("tempo = 132", "tempo = 400").replace("bars = 8", "bars = 1").replace("bars = 4", "bars = 1"))
    preferences = work / "config/com.phasecraft.player"
    preferences.mkdir(parents=True)
    (preferences / "recent.json").write_text(json.dumps([str(project)]))
    env = {**os.environ, "XDG_CONFIG_HOME": str(work / "config"), "WEBKIT_DISABLE_DMABUF_RENDERER": "1"}
    with (work / "driver.log").open("w") as log:
        process = subprocess.Popen(["tauri-driver", "--port", "4450", "--native-port", "4451"], env=env, stdout=log, stderr=log)
        driver = None
        try:
            options = ArgOptions()
            options.set_capability("tauri:options", {"application": str(Path(sys.argv[1]).resolve())})
            for attempt in range(30):
                try:
                    driver = webdriver.Remote(command_executor="http://127.0.0.1:4450", options=options)
                    break
                except Exception:
                    if attempt == 29:
                        raise
                    time.sleep(.2)
            wait = WebDriverWait(driver, 20)
            wait.until(lambda d: d.find_elements(By.CSS_SELECTOR, "#welcome-recent button"))
            driver.find_element(By.CSS_SELECTOR, "#welcome-recent button").click()
            wait.until(lambda d: len(d.find_elements(By.CSS_SELECTOR, ".part-card")) == 5)
            driver.find_element(By.ID, "settings-open").click()
            Select(driver.find_element(By.ID, "destination")).select_by_value("@silent")
            driver.find_element(By.ID, "settings-save").click()
            driver.find_element(By.ID, "projects-menu").click()
            driver.find_element(By.ID, "close-project").click()
            wait.until(lambda d: d.find_element(By.ID, "welcome").is_displayed())
            driver.find_element(By.CSS_SELECTOR, "#welcome-recent button").click()
            driver.find_element(By.ID, "settings-open").click()
            assert Select(driver.find_element(By.ID, "destination")).first_selected_option.get_attribute("value") == "@silent"
            driver.find_element(By.ID, "settings-dismiss").click()
            for _ in range(3):
                driver.find_element(By.ID, "play").click()
                wait.until(lambda d: "SILENT PREVIEW" in d.find_element(By.ID, "state").text)
                wait.until(lambda d: d.find_element(By.ID, "position").text != "1.1.1")
                driver.find_element(By.ID, "stop").click()
                wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "SILENT PREVIEW" in d.find_element(By.ID, "state").text)
            library = project / "patterns/drums.toml"
            original = library.read_text()
            library.write_text("invalid TOML !")
            wait.until(lambda d: "Playing the last valid system" in d.find_element(By.ID, "reload-error").text)
            assert "SILENT PREVIEW" in driver.find_element(By.ID, "state").text
            library.write_text(original)
            wait.until(lambda d: not d.find_element(By.ID, "reload-error").is_displayed())
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[1].click()
            wait.until(lambda d: d.find_element(By.ID, "tempo").text == "172")
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "SILENT PREVIEW" in d.find_element(By.ID, "state").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[2].click()
            wait.until(lambda d: len(d.find_elements(By.CSS_SELECTOR, ".part-card")) == 6)
            driver.find_element(By.ID, "settings-open").click()
            driver.find_element(By.ID, "send-clock").click()
            driver.find_element(By.ID, "settings-save").click()
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "SILENT PREVIEW" in d.find_element(By.ID, "state").text)
            driver.find_element(By.CSS_SELECTOR, '[data-part="closed_hat"]').click()
            wait.until(lambda d: "Timing +" in d.find_element(By.ID, "detail-body").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[3].click()
            wait.until(lambda d: len(d.find_elements(By.CSS_SELECTOR, ".part-card")) == 4)
            driver.find_element(By.ID, "play").click()
            driver.find_element(By.CSS_SELECTOR, '[data-part="rim"]').click()
            wait.until(lambda d: "CC 20" in d.find_element(By.ID, "detail-body").text)
            wait.until(lambda d: "CC 21" in d.find_element(By.ID, "detail-body").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[6].click()
            driver.find_element(By.ID, "play").click()
            driver.find_element(By.CSS_SELECTOR, '[data-part="closed_hat"]').click()
            wait.until(lambda d: "kit default restored" in d.find_element(By.ID, "detail-body").text)
            wait.until(lambda d: "CC 75" in d.find_element(By.ID, "detail-body").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[7].click()
            driver.find_element(By.ID, "play").click()
            driver.find_element(By.CSS_SELECTOR, '[data-part="closed_hat"]').click()
            wait.until(lambda d: all(name in d.find_element(By.ID, "detail-body").text for name in ["cutoff", "level", "pan", "decay"]))
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[10].click()
            driver.find_element(By.ID, "play").click()
            driver.find_element(By.CSS_SELECTOR, '[data-part="closed_hat"]').click()
            wait.until(lambda d: "Shared accent: drums" in d.find_element(By.ID, "detail-body").text)
            wait.until(lambda d: "envelope" in d.find_element(By.ID, "detail-body").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[11].click()
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "A · section 1/4" in d.find_element(By.ID, "section-position").text)
            driver.find_element(By.ID, "stop").click()
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            driver.find_elements(By.CSS_SELECTOR, "#compositions button")[14].click()
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "SILENT PREVIEW" in d.find_element(By.ID, "state").text)
            wait.until(lambda d: "STOPPED" in d.find_element(By.ID, "state").text)
            assert driver.find_element(By.ID, "position").text == "1.1.1"
            assert "Intro · section 1/5" in driver.find_element(By.ID, "section-position").text
            driver.find_element(By.ID, "play").click()
            wait.until(lambda d: "Intro · section 1/5" in d.find_element(By.ID, "section-position").text)
            # Closing the last window also kills WebDriver's session, so observe
            # the application process instead of asking the closed page to reply.
            application = Path(sys.argv[1]).resolve()
            app_processes = []
            for entry in Path("/proc").glob("[0-9]*"):
                try:
                    if (entry / "exe").resolve() == application:
                        if str(work / "config").encode() in (entry / "environ").read_bytes():
                            app_processes.append(entry)
                except (OSError, PermissionError):
                    pass
            assert len(app_processes) == 1
            driver.execute_script('setTimeout(() => document.getElementById("window-close").click(), 100)')
            wait.until(lambda _: not app_processes[0].exists())


            print("Native player: open, repeated playback, watched error/recovery, selection and close passed")
        except Exception:
            if driver:
                driver.save_screenshot(str(root / "desktop/test-results/native-failure.png"))
                print(driver.find_element(By.TAG_NAME, "body").text)
            raise
        finally:
            if driver:
                driver.quit()
            process.terminate()
            process.wait(timeout=10)
