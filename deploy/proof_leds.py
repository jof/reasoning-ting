# some-ting — Path A proof.
# If the firmware runs THIS file (because it lives on TINGDISK as main.py),
# the LEDs will visibly "chase" instead of the normal static boot pattern.
# Remove this file from TINGDISK to fall back to the stock app.
import ui, time

while True:
    for i in range(4):
        ui.leds(i, i)          # light fx column i + sample column i
        time.sleep_ms(120)
    ui.leds(-1, 0)             # brief "off-ish" frame
    time.sleep_ms(120)
