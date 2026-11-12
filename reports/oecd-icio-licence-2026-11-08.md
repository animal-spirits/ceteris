# OECD ICIO licence for the paid build (2026-11-08)

**Verdict: allowed in principle, with one open check.** OECD's general data terms permit commercial use, redistribution and adaptation with attribution. The ICIO-specific ReadMe could not be read from here and may add third-party notices. Not fully confirmed.

## Clauses (OECD Terms & Conditions, last updated 15 March 2024)
Source: https://www.oecd.org/termsandconditions (read via https://t4.oecd.org/termsandconditions; the www.oecd.org/en/about/terms-conditions.html page is JS-rendered and returned no body text).

- Use: "You can extract from, download, copy, adapt, print, distribute, share and embed Data for any purpose, even for commercial use."
- Attribution: "You must give appropriate credit to the OECD by using the citation associated with the relevant Data."
- Sub-licensing: must "include the same acknowledgment requirement in any sub-licenses that You grant".
- Third-party: users must check "whether the Data is fully or partially owned by third parties and/or whether additional restrictions may apply"; data "may be subject to restrictions beyond the scope of these Terms and Conditions" (quoted via https://social-science-data-editors.github.io/reference/information/OECD.html, terms as of 2023-10-08).
- ICIO page (https://t4.oecd.org/sti/ind/inter-country-input-output-tables.htm): no licence text. Citation: "OECD (2023), OECD Inter-Country Input-Output Database, http://oe.cd/icio". "Users are kindly asked to consult the ReadMe files beforehand."

## Ambiguity
- No licence statement specific to ICIO was found. It falls under the general terms unless the ReadMe says otherwise. ICIO is built from national accounts and national IO tables, so a third-party notice in the ReadMe is possible.
- "Even for commercial use" is explicit, but the PDF-posting ban and the copyright.com route apply to OECD publications ("Material"), not to Data. Shipping a derived binary pack is Data redistribution, but a lawyer-grade reading is not mine to give.
- Quotes are from mirrors/fetch summaries, not verbatim page views. Re-read in a browser before relying on them.

## Coverage vs needs
Repo sources.toml already records the 2026 edition: 2000-2023, 81 economies, 41 industries (ICIO 2025 grid). The 2023 edition was 1995-2020, 76 economies + ROW, 45 industries. Both cover the 2000 start. Coverage exceeds the 45-goods need at country level; industry split is coarser than EXIOBASE (163 products, 49 regions) but already used by the pack (`oecd-icio` entry).

## Replace EXIOBASE 3.9?
Licence-wise yes if the ReadMe is clean: it is the only candidate so far that is commercial-friendly. Loss: environmental extensions and finer industry detail. ICIO is already a pack source, so this is mostly a decision about which IO fields EXIOBASE uniquely supplied (check RFC 0314).

## Next step (Antonio's call)
1. Open the ICIO ReadMe (https://webfs-sti.oecd.org/files/STI-PIE/ICIO/2025/ReadMe_ICIO_small.xlsx, reachable per sources.toml) and look for third-party notices; if none, mark `oecd-icio` commercial-OK in sources.toml.
2. If clean, scope an RFC amendment to 0314 dropping EXIOBASE for the paid profile. No email needed unless the ReadMe flags a third party.
