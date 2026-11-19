# Hardware validation record

The [compatibility table](../README.md#hardware-what-works-and-what-is-only-a-candidate) separates observed lighting from simulated transport tests and identification without confirmed RGB. The scope of a result is the named controller, setup and operation, not every device sold under the same brand.

| Setup | Evidence in the repository | Still to confirm |
|---|---|---|
| Astral RTX 5080 OC, Crosshair VIII Hero (Wi-Fi) fixed Aura zone, two ENE DIMMs | The [earlier hardware report](https://github.com/LinRGB/LinRgb/blob/67ff099/README.md) explicitly lists solid color for all three. The [Aura audit](aura-usb.md) also records an interface descriptor read from the Crosshair. | An observed descriptor is not an effect test. The report does not establish suspend/resume or support for other cards or headers. |
| Strix B550-A: Corsair Pro SL and Aura header | The [beta.4 hardware summary](https://github.com/LinRGB/LinRgb/blob/v1.0.0-beta.4/README.md) records earlier SL direct color. The [Aura audit](aura-usb.md) records a header-to-GPU sync connection. | Test each restored DIMM session independently, capture the B550-A interface descriptor and verify the fixed and addressable outputs separately. No direct XFX GPU result is claimed. |
| Fractal Adjust Pro Hub | The [beta.4 summary](https://github.com/LinRGB/LinRgb/blob/v1.0.0-beta.4/README.md) records an earlier hardware setup. The [session audit](verified-sessions.md) distinguishes simulated WDL report tests from hardware results. | Repeated Apply and suspend/resume on the current session implementation. No physical lamp count or measured update rate is inferred from a fixture. |
| Corsair Pro (non-SL), Linux multicolor LEDs | [Corsair protocol tests](../src/drivers/corsair/tests.rs) and [session fixtures](../src/drivers/tests.rs) cover direct frames and sysfs writes respectively. | A named physical setup and observed color change. Passing tests or installing a package is not a physical result. |

To add a result, use the [tester guide](testing-beta.md). Record the LinRGB version or commit, exact controller and connection, selected target, operation and visible outcome. A diagnostics-only report is useful: mark RGB **not tested** rather than passed or failed.
