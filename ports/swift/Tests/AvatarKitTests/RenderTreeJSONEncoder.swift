import AvatarKit

// This encoder deliberately lives only in the test target. It is a verification
// instrument for proving Rust trees decode without loss, not a supported tree
// production API; shipping it would create a second source of truth for wire
// bytes beside the canonical Rust encoder.
func canonicalTreeJSONBytes(_ tree: RenderTree) -> [UInt8] {
    var writer = TreeJSONWriter()
    writer.write(tree)
    return Array(writer.output.utf8)
}

private struct TreeJSONWriter {
    var output = ""

    mutating func integer<T: BinaryInteger>(_ value: T) {
        output.append(String(value))
    }

    mutating func string(_ value: String) {
        output.append("\"")
        for scalar in value.unicodeScalars {
            switch scalar.value {
            case 0x08: output.append("\\b")
            case 0x09: output.append("\\t")
            case 0x0A: output.append("\\n")
            case 0x0C: output.append("\\f")
            case 0x0D: output.append("\\r")
            case 0x22: output.append("\\\"")
            case 0x5C: output.append("\\\\")
            case 0x00...0x1F:
                let hex = String(scalar.value, radix: 16, uppercase: false)
                output.append("\\u" + String(repeating: "0", count: 4 - hex.count) + hex)
            default:
                output.unicodeScalars.append(scalar)
            }
        }
        output.append("\"")
    }

    mutating func beginObject() -> Bool {
        output.append("{")
        return true
    }

    mutating func field(_ name: String, first: inout Bool) {
        if first {
            first = false
        } else {
            output.append(",")
        }
        string(name)
        output.append(":")
    }

    mutating func endObject() {
        output.append("}")
    }

    mutating func array<T>(_ values: [T], writeValue: (inout TreeJSONWriter, T) -> Void) {
        output.append("[")
        for (index, value) in values.enumerated() {
            if index != 0 { output.append(",") }
            writeValue(&self, value)
        }
        output.append("]")
    }

    mutating func optional<T>(_ value: T?, writeValue: (inout TreeJSONWriter, T) -> Void) {
        if let value {
            writeValue(&self, value)
        } else {
            output.append("null")
        }
    }

    mutating func write(_ tree: RenderTree) {
        var first = beginObject()
        field("background", first: &first)
        optional(tree.background) { $0.write($1) }
        field("canvas", first: &first)
        integer(tree.canvas)
        field("primitives", first: &first)
        array(tree.primitives) { $0.write($1) }
        endObject()
    }

    mutating func write(_ primitive: Primitive) {
        var first = beginObject()
        field("clip", first: &first)
        optional(primitive.clip) { $0.write($1) }
        field("effects", first: &first)
        array(primitive.effects) { $0.write($1) }
        field("label", first: &first)
        string(primitive.label)
        field("opacity", first: &first)
        optional(primitive.opacity) { $0.integer($1.value) }
        field("paint", first: &first)
        write(primitive.paint)
        field("role", first: &first)
        optional(primitive.role) { $0.string($1.rawValue) }
        field("shape", first: &first)
        write(primitive.shape)
        field("stroke", first: &first)
        optional(primitive.stroke) { $0.write($1) }
        field("transform", first: &first)
        optional(primitive.transform) { $0.write($1) }
        endObject()
    }

    mutating func write(_ shape: Shape) {
        switch shape {
        case .path(let segments, let closed, let fillRule):
            var first = beginObject()
            field("closed", first: &first)
            output.append(closed ? "true" : "false")
            field("fill_rule", first: &first)
            string(fillRule.rawValue)
            field("kind", first: &first)
            string("path")
            field("segments", first: &first)
            array(segments) { $0.write($1) }
            endObject()
        case .circle(let cx, let cy, let r):
            var first = beginObject()
            field("cx", first: &first); integer(cx)
            field("cy", first: &first); integer(cy)
            field("kind", first: &first); string("circle")
            field("r", first: &first); integer(r)
            endObject()
        case .ellipse(let cx, let cy, let rx, let ry):
            var first = beginObject()
            field("cx", first: &first); integer(cx)
            field("cy", first: &first); integer(cy)
            field("kind", first: &first); string("ellipse")
            field("rx", first: &first); integer(rx)
            field("ry", first: &first); integer(ry)
            endObject()
        case .rect(let x, let y, let w, let h):
            var first = beginObject()
            field("h", first: &first); integer(h)
            field("kind", first: &first); string("rect")
            field("w", first: &first); integer(w)
            field("x", first: &first); integer(x)
            field("y", first: &first); integer(y)
            endObject()
        }
    }

    mutating func write(_ segment: Segment) {
        switch segment {
        case .moveTo(let x, let y):
            var first = beginObject()
            field("kind", first: &first); string("move_to")
            field("x", first: &first); integer(x)
            field("y", first: &first); integer(y)
            endObject()
        case .cubicTo(let cp1x, let cp1y, let cp2x, let cp2y, let x, let y):
            var first = beginObject()
            field("cp1x", first: &first); integer(cp1x)
            field("cp1y", first: &first); integer(cp1y)
            field("cp2x", first: &first); integer(cp2x)
            field("cp2y", first: &first); integer(cp2y)
            field("kind", first: &first); string("cubic_to")
            field("x", first: &first); integer(x)
            field("y", first: &first); integer(y)
            endObject()
        case .lineTo(let x, let y):
            var first = beginObject()
            field("kind", first: &first); string("line_to")
            field("x", first: &first); integer(x)
            field("y", first: &first); integer(y)
            endObject()
        }
    }

    mutating func write(_ paint: Paint) {
        switch paint {
        case .solid(let color):
            var first = beginObject()
            field("color", first: &first); write(color)
            field("kind", first: &first); string("solid")
            endObject()
        case .radialGradient(let stops):
            var first = beginObject()
            field("kind", first: &first); string("radial_gradient")
            field("stops", first: &first)
            array(stops) { $0.write($1) }
            endObject()
        case .none:
            var first = beginObject()
            field("kind", first: &first); string("none")
            endObject()
        }
    }

    mutating func write(_ stop: GradientStop) {
        var first = beginObject()
        field("color", first: &first); write(stop.color)
        field("offset_q32", first: &first); integer(stop.offset_q32.value)
        endObject()
    }

    mutating func write(_ stroke: Stroke) {
        var first = beginObject()
        field("cap", first: &first); string(stroke.cap.rawValue)
        field("color", first: &first); write(stroke.color)
        field("join", first: &first); string(stroke.join.rawValue)
        field("width", first: &first); integer(stroke.width)
        endObject()
    }

    mutating func write(_ effect: Effect) {
        switch effect {
        case .lit(let lightY):
            var first = beginObject()
            field("kind", first: &first); string("lit")
            field("light_y", first: &first); integer(lightY)
            endObject()
        case .masked(let mask):
            var first = beginObject()
            field("kind", first: &first); string("masked")
            field("mask", first: &first); array(mask) { $0.write($1) }
            endObject()
        case .turbulence(let seed, let freqH, let freqV, let octaves, let scale):
            var first = beginObject()
            field("freq_h", first: &first); integer(freqH.value)
            field("freq_v", first: &first); integer(freqV.value)
            field("kind", first: &first); string("turbulence")
            field("octaves", first: &first); integer(octaves)
            field("scale", first: &first); integer(scale.value)
            field("seed", first: &first); integer(seed)
            endObject()
        }
    }

    mutating func write(_ transform: Transform) {
        var first = beginObject()
        field("ops", first: &first)
        array(transform.ops) { $0.write($1) }
        endObject()
    }

    mutating func write(_ operation: TransformOp) {
        switch operation {
        case .translate(let tx, let ty):
            var first = beginObject()
            field("kind", first: &first); string("translate")
            field("tx", first: &first); integer(tx)
            field("ty", first: &first); integer(ty)
            endObject()
        case .rotate(let angle, let cx, let cy):
            var first = beginObject()
            field("angle", first: &first); integer(angle.value)
            field("cx", first: &first); integer(cx)
            field("cy", first: &first); integer(cy)
            field("kind", first: &first); string("rotate")
            endObject()
        case .scale(let sx, let sy):
            var first = beginObject()
            field("kind", first: &first); string("scale")
            field("sx", first: &first); integer(sx.value)
            field("sy", first: &first); integer(sy.value)
            endObject()
        }
    }

    mutating func write(_ color: Rgb) {
        var first = beginObject()
        field("b", first: &first); integer(color.b)
        field("g", first: &first); integer(color.g)
        field("r", first: &first); integer(color.r)
        endObject()
    }
}
