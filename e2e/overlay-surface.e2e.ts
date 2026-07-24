import { expect, test } from "@playwright/test";

test("toolbar and note documents are transparent outside their cards", async ({
  page
}) => {
  for (const overlay of ["toolbar", "note"]) {
    await page.goto(`/?overlay=${overlay}`);
    const surface = await page.evaluate(() => {
      const root = document.getElementById("root")!;
      return {
        className: document.documentElement.className,
        html: getComputedStyle(document.documentElement).backgroundColor,
        body: getComputedStyle(document.body).backgroundColor,
        root: getComputedStyle(root).backgroundColor,
        minWidth: getComputedStyle(document.body).minWidth,
        overflow: getComputedStyle(root).overflow
      };
    });

    expect(surface).toEqual({
      className: "overlay-surface",
      html: "rgba(0, 0, 0, 0)",
      body: "rgba(0, 0, 0, 0)",
      root: "rgba(0, 0, 0, 0)",
      minWidth: "0px",
      overflow: "hidden"
    });
  }
});

test("main and unsupported routes preserve the opaque application surface", async ({
  page
}) => {
  for (const path of ["/", "/?overlay=unsupported"]) {
    await page.goto(path);

    await expect(page.locator("html")).not.toHaveClass("overlay-surface");
    expect(
      await page.locator("html").evaluate((element) => {
        return getComputedStyle(element).backgroundColor;
      })
    ).toBe("rgb(238, 241, 245)");
  }
});
