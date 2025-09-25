import * as recast from "recast";

export function applyJsProxyOnJs(input: string): string {
    const ast = recast.parse(input);
    recast.types.visit(ast, {
        visitIdentifier(path) {
            if (path.node.name === "window") {
                path.replace(recast.types.builders.identifier("proxiedWindow"));
                return false;
            } else if (path.node.name === "document") {
                path.replace(recast.types.builders.identifier("proxiedDocument"));
                return false;
            } else if (path.node.name === "location") {
                path.replace(recast.types.builders.identifier("proxiedLocation"));
                return false;
            // } else if (path.node.name === "self") {
            //     path.replace(recast.types.builders.identifier("proxiedSelf"));
            //     return false;
            // } else if (path.node.name === "globalThis") {
            //     path.replace(recast.types.builders.identifier("proxiedGlobalThis"));
            //     return false;
            }

            this.traverse(path);
        },
    });
    
    return recast.print(ast).code;
}

export function setupScripts() {
    const originalInnerHTMLDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "innerHTML");
    const originalInnerTextDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "innerText");

    console.log("Setting up script proxying");

    function proxiedSetter(originalDescriptor: PropertyDescriptor | undefined, element: Element, value: string, propName: string) {
        if (element.tagName === "SCRIPT") {
            try {
                // Transform the script content before setting it
                value = applyJsProxyOnJs(value);
                console.log("applied js proxy to script content");
            } catch (err) {
                console.error(`Error applying JS proxy to script content: ${err}`);
            }
        }

        if (originalDescriptor && originalDescriptor.set) {
            originalDescriptor.set.call(element, value);
        } else {
            element[propName] = value;
        }
    }

    Object.defineProperty(Element.prototype, "innerHTML", {
        set(value: string) {
            proxiedSetter(originalInnerHTMLDescriptor, this, value, "innerHTML");
        },
        get() {
            return originalInnerHTMLDescriptor && originalInnerHTMLDescriptor.get
                ? originalInnerHTMLDescriptor.get.call(this)
                : this.innerHTML;
        },
        configurable: true,
        enumerable: true,
    });

    Object.defineProperty(Element.prototype, "innerText", {
        set(value: string) {
            proxiedSetter(originalInnerTextDescriptor, this, value, "innerText");
        },
        get() {
            return originalInnerTextDescriptor && originalInnerTextDescriptor.get
                ? originalInnerTextDescriptor.get.call(this)
                : this.innerText;
        },
        configurable: true,
        enumerable: true,
    });
}
